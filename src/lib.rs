//! Rust port of the core `pkg/ahap` Go package: builds and serializes
//! Apple Haptic and Audio Pattern (AHAP) files.
//!
//! Deliberately idiomatic, not a line-for-line translation: envelope/curve
//! builders consume `self` and return owned values instead of mutating a
//! shared `*Builder` the way the Go fluent API does, which sidesteps
//! borrow-checker friction entirely.

use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

pub mod curves;
pub mod musical;
pub mod builder;

pub use curves::*;
pub use musical::*;
pub use builder::*;

// ---- Parameter ID constants (mirrors the Go package's string constants) ----

/// `EventType` values for the `Event` struct.
pub const EVENT_TYPE_HAPTIC_TRANSIENT: &str = "HapticTransient";
pub const EVENT_TYPE_HAPTIC_CONTINUOUS: &str = "HapticContinuous";
pub const EVENT_TYPE_AUDIO_CUSTOM: &str = "AudioCustom";
pub const EVENT_TYPE_AUDIO_CONTINUOUS: &str = "AudioContinuous";

/// Static `ParameterID` values for an `Event`'s `EventParameters`.
pub const PARAM_HAPTIC_INTENSITY: &str = "HapticIntensity";
pub const PARAM_HAPTIC_SHARPNESS: &str = "HapticSharpness";
pub const PARAM_HAPTIC_ATTACK_TIME: &str = "HapticAttackTime";
pub const PARAM_HAPTIC_DECAY_TIME: &str = "HapticDecayTime";
pub const PARAM_HAPTIC_RELEASE_TIME: &str = "HapticReleaseTime";

/// Dynamic `ParameterID` values for a `ParameterCurve` - these modulate an
/// already-playing event's intensity/sharpness as a multiplier over time,
/// rather than setting a fixed value.
pub const CURVE_HAPTIC_INTENSITY: &str = "HapticIntensityControl";
pub const CURVE_HAPTIC_SHARPNESS: &str = "HapticSharpnessControl";

// ---- Core data model (field names match the AHAP spec / Go json tags) ----

/// A complete AHAP document: version, metadata, and the pattern (the
/// ordered list of events and parameter curves).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ahap {
    #[serde(rename = "Version")]
    pub version: f64,
    #[serde(rename = "Metadata")]
    pub metadata: Metadata,
    #[serde(rename = "Pattern")]
    pub pattern: Vec<PatternItem>,
}

/// Informational-only metadata about an AHAP file. Not read by Apple's
/// player; useful for humans/tooling inspecting the file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    #[serde(rename = "Project")]
    pub project: String,
    #[serde(rename = "Created")]
    pub created: String,
    #[serde(rename = "Description")]
    pub description: String,
    #[serde(rename = "Created By")]
    pub created_by: String,
}

/// Mirrors the Go `Pattern` struct: exactly one of `event` / `parameter_curve`
/// is set per AHAP spec, modeled with `Option` (not an enum) so the JSON shape
/// stays identical to what Apple's own examples and the Go implementation emit.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PatternItem {
    #[serde(rename = "Event", skip_serializing_if = "Option::is_none")]
    pub event: Option<Event>,
    #[serde(rename = "ParameterCurve", skip_serializing_if = "Option::is_none")]
    pub parameter_curve: Option<ParameterCurve>,
}

/// A single haptic or audio event: a transient, a continuous, or an audio
/// event, depending on `event_type`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    #[serde(rename = "Time")]
    pub time: f64,
    #[serde(rename = "EventType")]
    pub event_type: String,
    #[serde(rename = "EventParameters")]
    pub event_parameters: Vec<EventParameter>,
    #[serde(rename = "EventDuration", skip_serializing_if = "Option::is_none")]
    pub event_duration: Option<f64>,
    #[serde(rename = "EventWaveformPath", skip_serializing_if = "Option::is_none")]
    pub event_waveform_path: Option<String>,
}

/// One `(id, value)` pair inside an `Event`'s `EventParameters` - e.g.
/// `HapticIntensity` / `0.8`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventParameter {
    #[serde(rename = "ParameterID")]
    pub parameter_id: String,
    #[serde(rename = "ParameterValue")]
    pub parameter_value: f64,
}

/// A dynamic parameter curve: modulates `parameter_id` on an already-playing
/// event starting at `time`, following `control_points` (each point's `time`
/// is relative to this curve's own `time`, not absolute).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParameterCurve {
    #[serde(rename = "ParameterID")]
    pub parameter_id: String,
    #[serde(rename = "Time")]
    pub time: f64,
    #[serde(rename = "ParameterCurveControlPoints")]
    pub control_points: Vec<ControlPoint>,
}

/// One point on a [`ParameterCurve`]: a relative time and the parameter
/// value at that point, interpolated between neighboring points.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControlPoint {
    #[serde(rename = "Time")]
    pub time: f64,
    #[serde(rename = "ParameterValue")]
    pub parameter_value: f64,
}

/// Optional Attack/Decay/Release envelope, shared by transient and continuous
/// event constructors. `None` fields are omitted from the emitted JSON, same
/// as the Go `*float64` + nil-check approach.
#[derive(Debug, Clone, Copy, Default)]
pub struct Envelope {
    pub attack: Option<f64>,
    pub decay: Option<f64>,
    pub release: Option<f64>,
}

impl Envelope {
    /// No envelope shaping - all three fields omitted from the emitted JSON.
    pub fn none() -> Self {
        Self::default()
    }

    fn to_parameters(self) -> Vec<EventParameter> {
        let mut params = Vec::with_capacity(3);
        if let Some(a) = self.attack {
            params.push(EventParameter { parameter_id: PARAM_HAPTIC_ATTACK_TIME.into(), parameter_value: a });
        }
        if let Some(d) = self.decay {
            params.push(EventParameter { parameter_id: PARAM_HAPTIC_DECAY_TIME.into(), parameter_value: d });
        }
        if let Some(r) = self.release {
            params.push(EventParameter { parameter_id: PARAM_HAPTIC_RELEASE_TIME.into(), parameter_value: r });
        }
        params
    }
}

impl Ahap {
    /// Starts a new, empty AHAP document (version 1.0, no pattern events yet).
    pub fn new(description: impl Into<String>, created_by: impl Into<String>) -> Self {
        Self {
            version: 1.0,
            metadata: Metadata {
                project: "Basis".into(),
                created: timestamp_now(),
                description: description.into(),
                created_by: created_by.into(),
            },
            pattern: Vec::new(),
        }
    }

    /// Appends an already-built [`Event`] (see [`crate::Transient`]/[`crate::Continuous`]).
    pub fn add_event(&mut self, event: Event) {
        self.pattern.push(PatternItem { event: Some(event), parameter_curve: None });
    }

    /// Appends an already-built [`ParameterCurve`] (see [`crate::Curve`]).
    pub fn add_parameter_curve(&mut self, curve: ParameterCurve) {
        self.pattern.push(PatternItem { event: None, parameter_curve: Some(curve) });
    }

    /// Adds a plain `HapticTransient` event with no envelope shaping.
    pub fn add_haptic_transient(&mut self, time: f64, intensity: f64, sharpness: f64) {
        self.add_haptic_transient_envelope(time, intensity, sharpness, Envelope::none());
    }

    /// Adds a `HapticTransient` event with optional attack/decay/release
    /// envelope shaping (see [`Envelope`]).
    pub fn add_haptic_transient_envelope(&mut self, time: f64, intensity: f64, sharpness: f64, env: Envelope) {
        let mut params = vec![
            EventParameter { parameter_id: PARAM_HAPTIC_INTENSITY.into(), parameter_value: intensity },
            EventParameter { parameter_id: PARAM_HAPTIC_SHARPNESS.into(), parameter_value: sharpness },
        ];
        params.extend(env.to_parameters());
        self.add_event(Event {
            time,
            event_type: EVENT_TYPE_HAPTIC_TRANSIENT.into(),
            event_parameters: params,
            event_duration: None,
            event_waveform_path: None,
        });
    }

    /// Adds a plain `HapticContinuous` event with no envelope shaping.
    pub fn add_haptic_continuous(&mut self, time: f64, duration: f64, intensity: f64, sharpness: f64) {
        self.add_haptic_continuous_envelope(time, duration, intensity, sharpness, Envelope::none());
    }

    /// Adds a `HapticContinuous` event with optional attack/decay/release
    /// envelope shaping (see [`Envelope`]).
    pub fn add_haptic_continuous_envelope(
        &mut self,
        time: f64,
        duration: f64,
        intensity: f64,
        sharpness: f64,
        env: Envelope,
    ) {
        let mut params = vec![
            EventParameter { parameter_id: PARAM_HAPTIC_INTENSITY.into(), parameter_value: intensity },
            EventParameter { parameter_id: PARAM_HAPTIC_SHARPNESS.into(), parameter_value: sharpness },
        ];
        params.extend(env.to_parameters());
        self.add_event(Event {
            time,
            event_type: EVENT_TYPE_HAPTIC_CONTINUOUS.into(),
            event_parameters: params,
            event_duration: Some(duration),
            event_waveform_path: None,
        });
    }

    /// Adds a parameter curve from raw control points. Prefer [`crate::Curve`]
    /// for building `control_points` with interpolation helpers.
    pub fn add_curve(&mut self, parameter_id: impl Into<String>, start_time: f64, control_points: Vec<ControlPoint>) {
        self.add_parameter_curve(ParameterCurve {
            parameter_id: parameter_id.into(),
            time: start_time,
            control_points,
        });
    }

    /// Serializes to a JSON string, pretty-printed if `indent` is true.
    pub fn to_json(&self, indent: bool) -> serde_json::Result<String> {
        if indent {
            serde_json::to_string_pretty(self)
        } else {
            serde_json::to_string(self)
        }
    }

    /// Serializes and writes to `path`.
    pub fn export(&self, path: impl AsRef<Path>, indent: bool) -> io::Result<()> {
        let data = self
            .to_json(indent)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, data)
    }
}

/// Timestamp string. Not date-formatted like Go's layout (that needs chrono),
/// but monotonic/unique and human-readable enough for AHAP Metadata, which
/// Apple's player never actually parses.
fn timestamp_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{}.{:06}", now.as_secs(), now.subsec_micros())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_sets_defaults() {
        let a = Ahap::new("test description", "test creator");
        assert_eq!(a.version, 1.0);
        assert_eq!(a.metadata.description, "test description");
        assert_eq!(a.metadata.created_by, "test creator");
        assert!(a.pattern.is_empty());
    }

    #[test]
    fn add_haptic_transient() {
        let mut a = Ahap::new("t", "t");
        a.add_haptic_transient(0.5, 1.0, 0.8);
        assert_eq!(a.pattern.len(), 1);
        let event = a.pattern[0].event.as_ref().unwrap();
        assert_eq!(event.time, 0.5);
        assert_eq!(event.event_type, EVENT_TYPE_HAPTIC_TRANSIENT);
        assert_eq!(event.event_parameters.len(), 2);
    }

    #[test]
    fn add_haptic_continuous() {
        let mut a = Ahap::new("t", "t");
        a.add_haptic_continuous(1.0, 2.0, 0.7, 0.6);
        let event = a.pattern[0].event.as_ref().unwrap();
        assert_eq!(event.event_type, EVENT_TYPE_HAPTIC_CONTINUOUS);
        assert_eq!(event.event_duration, Some(2.0));
    }

    #[test]
    fn envelope_omits_nil_fields() {
        let mut a = Ahap::new("t", "t");
        a.add_haptic_continuous_envelope(0.0, 0.2, 1.0, 0.5, Envelope::none());
        let event = a.pattern[0].event.as_ref().unwrap();
        assert_eq!(event.event_parameters.len(), 2, "no envelope params should be emitted");
    }

    #[test]
    fn envelope_includes_set_fields_only() {
        let mut a = Ahap::new("t", "t");
        let env = Envelope { attack: Some(0.01), decay: Some(0.05), release: None };
        a.add_haptic_transient_envelope(0.0, 1.0, 0.5, env);
        let event = a.pattern[0].event.as_ref().unwrap();
        assert_eq!(event.event_parameters.len(), 4);
        assert!(event.event_parameters.iter().any(|p| p.parameter_id == PARAM_HAPTIC_ATTACK_TIME));
        assert!(event.event_parameters.iter().any(|p| p.parameter_id == PARAM_HAPTIC_DECAY_TIME));
        assert!(!event.event_parameters.iter().any(|p| p.parameter_id == PARAM_HAPTIC_RELEASE_TIME));
    }

    #[test]
    fn create_curve_matches_expected_endpoints() {
        let points = create_curve(0.0, 1.0, 0.0, 1.0, 10);
        assert_eq!(points.len(), 10);
        assert!((points[0].time - 0.1).abs() < 1e-9);
        assert!((points[9].time - 1.0).abs() < 1e-9);
        assert!((points[0].parameter_value - 0.1).abs() < 1e-9);
        assert!((points[9].parameter_value - 1.0).abs() < 1e-9);
    }

    #[test]
    fn freq_to_sharpness_range_and_clamping() {
        assert!(freq_to_sharpness(80.0, false).is_ok());
        assert!(freq_to_sharpness(230.0, false).is_ok());
        assert!(freq_to_sharpness(79.0, false).is_err());
        assert!(freq_to_sharpness(231.0, false).is_err());
        assert!(freq_to_sharpness(50.0, true).is_ok()); // clamped to 80
        assert!(freq_to_sharpness(300.0, true).is_ok()); // clamped to 230
        let r = freq_to_sharpness(155.0, false).unwrap();
        assert!((0.0..=1.0).contains(&r));
    }

    #[test]
    fn musical_context_matches_expected_timings() {
        let mc = MusicalContext::new(120.0, 4, 4);
        assert_eq!(mc.beat_duration(), 0.5);
        assert_eq!(mc.bar_duration(), 2.0);
        assert_eq!(mc.beat_to_seconds(4.0), 2.0);
        assert_eq!(mc.bar_to_seconds(2.0), 4.0);
        assert_eq!(mc.at(1, 0), 2.0); // bar 1, beat 0 at 120 BPM 4/4
    }

    #[test]
    fn builder_transient_and_continuous_roundtrip_json() {
        let mut a = Ahap::new("test", "test creator");
        a.add_event(Transient::at(0.0).intensity(1.0).sharpness(0.5).build());
        a.add_event(Continuous::at(1.0, 2.0).intensity(0.8).sharpness(0.7).build());
        assert_eq!(a.pattern.len(), 2);

        let json = a.to_json(true).unwrap();
        let decoded: Ahap = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.version, 1.0);
        assert_eq!(decoded.pattern.len(), 2);
    }

    #[test]
    fn curve_builder_anchor_plus_ramp() {
        let curve = Curve::new(CURVE_HAPTIC_INTENSITY, 0.5)
            .anchor(0.0, 1.0)
            .ease_in_out_to((0.0, 1.0), (0.6, 0.0), 6)
            .build();
        assert_eq!(curve.control_points.len(), 7); // 1 anchor + 6 ramp points
        assert_eq!(curve.control_points[0].time, 0.0);
        assert_eq!(curve.control_points[0].parameter_value, 1.0);
        let last = curve.control_points.last().unwrap();
        assert!((last.time - 0.6).abs() < 1e-9);
        assert!(last.parameter_value.abs() < 1e-9);
    }

    #[test]
    fn export_writes_valid_json_file() {
        let mut a = Ahap::new("test export", "test creator");
        a.add_haptic_transient(0.0, 1.0, 0.5);
        let path = std::env::temp_dir().join("ahap_rs_test_export.ahap");
        a.export(&path, true).unwrap();

        let data = fs::read_to_string(&path).unwrap();
        let decoded: Ahap = serde_json::from_str(&data).unwrap();
        assert_eq!(decoded.version, 1.0);
        assert_eq!(decoded.pattern.len(), 1);
        let _ = fs::remove_file(&path);
    }
}
