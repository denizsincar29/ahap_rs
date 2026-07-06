//! Fluent, consuming-self builders for the two haptic event types
//! (`Transient`, `Continuous`) and for parameter curves, plus a higher-level
//! [`Builder`] that adds musical (bar/beat) addressing on top.

use crate::{ControlPoint, Envelope, Event, EVENT_TYPE_HAPTIC_CONTINUOUS, EVENT_TYPE_HAPTIC_TRANSIENT};
use crate::{EventParameter, ParameterCurve, PARAM_HAPTIC_INTENSITY, PARAM_HAPTIC_SHARPNESS};
use crate::{ease_in_out, exponential, create_curve};
use crate::Ahap;

/// Consuming-self builder for a `HapticTransient` event. Unlike the Go
/// `TransientBuilder`, this doesn't hold a back-reference to the parent
/// `Ahap`/`Builder` - it just builds an `Event`, which the caller then
/// hands to `Ahap::add_event`. That sidesteps needing a lifetime parameter
/// tying this builder to a `&mut Ahap` for the entire chain.
#[derive(Debug, Clone, Copy)]
pub struct Transient {
    time: f64,
    intensity: f64,
    sharpness: f64,
    envelope: Envelope,
}

impl Transient {
    /// Starts a transient event at `time` seconds, with default
    /// intensity/sharpness of 0.5 and no envelope.
    pub fn at(time: f64) -> Self {
        Self { time, intensity: 0.5, sharpness: 0.5, envelope: Envelope::none() }
    }

    pub fn intensity(mut self, v: f64) -> Self {
        self.intensity = v;
        self
    }

    pub fn sharpness(mut self, v: f64) -> Self {
        self.sharpness = v;
        self
    }

    /// Sets the optional `HapticAttackTime` envelope parameter.
    pub fn attack(mut self, v: f64) -> Self {
        self.envelope.attack = Some(v);
        self
    }

    /// Sets the optional `HapticDecayTime` envelope parameter.
    pub fn decay(mut self, v: f64) -> Self {
        self.envelope.decay = Some(v);
        self
    }

    /// Sets the optional `HapticReleaseTime` envelope parameter.
    pub fn release(mut self, v: f64) -> Self {
        self.envelope.release = Some(v);
        self
    }

    /// Finishes the builder into an [`Event`], ready for [`Ahap::add_event`].
    pub fn build(self) -> Event {
        let mut params = vec![
            EventParameter { parameter_id: PARAM_HAPTIC_INTENSITY.into(), parameter_value: self.intensity },
            EventParameter { parameter_id: PARAM_HAPTIC_SHARPNESS.into(), parameter_value: self.sharpness },
        ];
        params.extend(self.envelope.to_parameters());
        Event {
            time: self.time,
            event_type: EVENT_TYPE_HAPTIC_TRANSIENT.into(),
            event_parameters: params,
            event_duration: None,
            event_waveform_path: None,
        }
    }
}

/// Consuming-self builder for a `HapticContinuous` event.
#[derive(Debug, Clone, Copy)]
pub struct Continuous {
    time: f64,
    duration: f64,
    intensity: f64,
    sharpness: f64,
    envelope: Envelope,
}

impl Continuous {
    /// Starts a continuous event at `time` seconds lasting `duration`
    /// seconds, with default intensity/sharpness of 0.5 and no envelope.
    pub fn at(time: f64, duration: f64) -> Self {
        Self { time, duration, intensity: 0.5, sharpness: 0.5, envelope: Envelope::none() }
    }

    pub fn intensity(mut self, v: f64) -> Self {
        self.intensity = v;
        self
    }

    pub fn sharpness(mut self, v: f64) -> Self {
        self.sharpness = v;
        self
    }

    /// Sets the optional `HapticAttackTime` envelope parameter.
    pub fn attack(mut self, v: f64) -> Self {
        self.envelope.attack = Some(v);
        self
    }

    /// Sets the optional `HapticDecayTime` envelope parameter.
    pub fn decay(mut self, v: f64) -> Self {
        self.envelope.decay = Some(v);
        self
    }

    /// Sets the optional `HapticReleaseTime` envelope parameter.
    pub fn release(mut self, v: f64) -> Self {
        self.envelope.release = Some(v);
        self
    }

    /// Finishes the builder into an [`Event`], ready for [`Ahap::add_event`].
    pub fn build(self) -> Event {
        let mut params = vec![
            EventParameter { parameter_id: PARAM_HAPTIC_INTENSITY.into(), parameter_value: self.intensity },
            EventParameter { parameter_id: PARAM_HAPTIC_SHARPNESS.into(), parameter_value: self.sharpness },
        ];
        params.extend(self.envelope.to_parameters());
        Event {
            time: self.time,
            event_type: EVENT_TYPE_HAPTIC_CONTINUOUS.into(),
            event_parameters: params,
            event_duration: Some(self.duration),
            event_waveform_path: None,
        }
    }
}

/// Builder for a parameter curve (e.g. a decay ramp on `HapticIntensityControl`).
/// `anchor` is always emitted first at relative time 0 so playback doesn't
/// depend on an implicit "hold" before the first generated point - see the
/// long comment in `cmd/midi2ahap`'s Go equivalent for why that matters.
pub struct Curve {
    parameter_id: String,
    start_time: f64,
    points: Vec<ControlPoint>,
}

impl Curve {
    /// Starts a curve for `parameter_id` (e.g. [`crate::CURVE_HAPTIC_INTENSITY`])
    /// beginning at `start_time` seconds.
    pub fn new(parameter_id: impl Into<String>, start_time: f64) -> Self {
        Self { parameter_id: parameter_id.into(), start_time, points: Vec::new() }
    }

    /// Adds a single fixed control point at `relative_time` (relative to the
    /// curve's own `start_time`) with `value`. Typically used to pin down
    /// the curve's starting value before appending an interpolated ramp.
    pub fn anchor(mut self, relative_time: f64, value: f64) -> Self {
        self.points.push(ControlPoint { time: relative_time, parameter_value: value });
        self
    }

    /// Appends a smoothstep-interpolated ramp from `from` to `to`
    /// (each a `(relative_time, value)` pair), in `steps` points.
    pub fn ease_in_out_to(mut self, from: (f64, f64), to: (f64, f64), steps: usize) -> Self {
        let start = ControlPoint { time: from.0, parameter_value: from.1 };
        let end = ControlPoint { time: to.0, parameter_value: to.1 };
        self.points.extend(ease_in_out(start, end, steps));
        self
    }

    /// Appends a power-curve-interpolated ramp from `from` to `to`.
    pub fn exponential_to(mut self, from: (f64, f64), to: (f64, f64), steps: usize, exponent: f64) -> Self {
        let start = ControlPoint { time: from.0, parameter_value: from.1 };
        let end = ControlPoint { time: to.0, parameter_value: to.1 };
        self.points.extend(exponential(start, end, steps, exponent));
        self
    }

    /// Appends a linearly-interpolated ramp from `from` to `to`.
    pub fn linear_to(mut self, from: (f64, f64), to: (f64, f64), steps: usize) -> Self {
        self.points.extend(create_curve(from.0, to.0, from.1, to.1, steps));
        self
    }

    /// Finishes the builder into a [`ParameterCurve`], ready for
    /// [`Ahap::add_parameter_curve`].
    pub fn build(self) -> ParameterCurve {
        ParameterCurve { parameter_id: self.parameter_id, time: self.start_time, control_points: self.points }
    }
}

/// Higher-level builder wrapping an `Ahap` plus an optional `MusicalContext`,
/// mirroring Go's `*ahap.Builder`. Used by the DSL/interactive front-ends
/// (haptrack, ahapgen, makeahap) that want bar/beat addressing and a
/// slightly less verbose call surface than constructing `Event`s by hand.
pub struct Builder {
    ahap: Ahap,
    musical: Option<crate::MusicalContext>,
}

impl Builder {
    pub fn new(description: impl Into<String>, creator: impl Into<String>) -> Self {
        Self { ahap: Ahap::new(description, creator), musical: None }
    }

    /// Enables bar/beat addressing at this BPM (defaulting to 4/4 if no
    /// time signature has been set yet).
    pub fn with_bpm(mut self, bpm: f64) -> Self {
        let (num, den) = self.musical.map(|m| (m.time_signature.numerator, m.time_signature.denominator)).unwrap_or((4, 4));
        self.musical = Some(crate::MusicalContext::new(bpm, num, den));
        self
    }

    /// Sets the time signature used for bar/beat addressing (defaulting to
    /// 120 BPM if no BPM has been set yet).
    pub fn with_time_signature(mut self, numerator: u32, denominator: u32) -> Self {
        let bpm = self.musical.map(|m| m.bpm).unwrap_or(120.0);
        self.musical = Some(crate::MusicalContext::new(bpm, numerator, denominator));
        self
    }

    /// Beats per bar (4 if no time signature has been set).
    pub fn beats_per_bar(&self) -> u32 {
        self.musical.map(|m| m.beats_per_bar()).unwrap_or(4)
    }

    /// Absolute time in seconds for (bar, beat). Requires `with_bpm` to have
    /// been called; otherwise falls back to 120 BPM / 4/4 like the Go default.
    pub fn at(&self, bar: i64, beat: i64) -> f64 {
        self.musical.unwrap_or(crate::MusicalContext::new(120.0, 4, 4)).at(bar, beat)
    }

    /// Adds a plain transient event; use [`Transient`] directly for envelope shaping.
    pub fn add_transient(&mut self, time: f64, intensity: f64, sharpness: f64) {
        self.ahap.add_event(Transient::at(time).intensity(intensity).sharpness(sharpness).build());
    }

    /// Adds a plain continuous event; use [`Continuous`] directly for envelope shaping.
    pub fn add_continuous(&mut self, time: f64, duration: f64, intensity: f64, sharpness: f64) {
        self.ahap.add_event(Continuous::at(time, duration).intensity(intensity).sharpness(sharpness).build());
    }

    /// Adds a parameter curve from a list of (relative_time, value) points,
    /// linearly interpolated in `steps` increments between each consecutive
    /// pair (matching the Go builder's `.From(...).To(...).Steps(n)` shape).
    pub fn add_curve(&mut self, parameter_id: impl Into<String>, start_time: f64, points: Vec<(f64, f64)>, steps: usize) {
        let mut curve = Curve::new(parameter_id, start_time);
        for pair in points.windows(2) {
            curve = curve.linear_to(pair[0], pair[1], steps);
        }
        self.ahap.add_parameter_curve(curve.build());
    }

    /// Consumes the builder into the finished [`Ahap`].
    pub fn build(self) -> Ahap {
        self.ahap
    }

    /// Exports without consuming the builder, for callers (like an
    /// interactive REPL) that might need to keep using it if the export
    /// fails or before deciding to exit.
    pub fn export(&self, path: impl AsRef<std::path::Path>, indent: bool) -> std::io::Result<()> {
        self.ahap.export(path, indent)
    }
}
