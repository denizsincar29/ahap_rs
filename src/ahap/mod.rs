use chrono::prelude::*;
use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf};

fn round3(a: f32) -> f32 {
    (a*1000.0).round()/1000.0
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub enum CurveParamID {
    HapticIntensityControl,
    HapticSharpnessControl,
    HapticAttackTimeControl,
    HapticDecayTimeControl,
    HapticReleaseTimeControl,
    AudioBrightnessControl,
    AudioPanControl,
    AudioPitchControl,
    AudioVolumeControl,
    AudioAttackTimeControl,
    AudioDecayTimeControl,
    AudioReleaseTimeControl,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub enum ParamID {
    HapticIntensity,
    HapticSharpness,
    HapticAttackTime,
    HapticDecayTime,
    HapticReleaseTime,
    AudioBrightness,
    AudioPan,
    AudioPitch,
    AudioVolume,
    AudioAttackTime,
    AudioDecayTime,
    AudioReleaseTime,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct HapticCurve {
    pub time: f32,
    pub parameter_value: f32,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct AHAP {
    version: f32,
    metadata: Metadata,
    pattern: Vec<EventPattern>,
}
// all must be camel case
#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Metadata {
    project: String,
    created: String,
    description: String,
    // created by is separated by a space, which makes this a bit unusual
    #[serde(rename = "Created By")]
    created_by: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct EventPattern {
    #[serde(skip_serializing_if = "Option::is_none")]
    event: Option<Event>,
    #[serde(skip_serializing_if = "Option::is_none")]
    parameter_curve: Option<ParameterCurve>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Event {
    time: f32,
    event_type: String,
    event_parameters: Vec<Parameter>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_duration: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    event_waveform_path: Option<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct ParameterCurve {
    #[serde(rename = "ParameterID")]
    parameter_id: CurveParamID,
    time: f32,
    parameter_curve_control_points: Vec<HapticCurve>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
pub struct Parameter {
    #[serde(rename = "ParameterID")]
    parameter_id: ParamID,
    parameter_value: f32,
}

impl AHAP {
    pub fn new(description: &str, created_by: &str) -> Self {
        AHAP {
            version: 1.0,
            metadata: Metadata {
                project: "Basis".to_string(),
                created: Utc::now().to_rfc3339(),
                description: description.to_string(),
                created_by: created_by.to_string(),
            },
            pattern: Vec::new(),
        }
    }

    pub fn add_event(&mut self, etype: &str, time: f32, parameters: Vec<Parameter>, event_duration: Option<f32>, event_waveform_path: Option<String>) {
        self.pattern.push(EventPattern {
            event: Some(Event {
                time: time,
                event_type: etype.to_string(),
                event_parameters: parameters,
                event_duration,
                event_waveform_path,
            }),
            parameter_curve: None,
        });
    }

    pub fn add_haptic_transient_event(&mut self, time: f32, haptic_intensity: f32, haptic_sharpness: f32) {
        let parameters = vec![
            Parameter {
                parameter_id: ParamID::HapticIntensity,
                parameter_value: haptic_intensity,
            },
            Parameter {
                parameter_id: ParamID::HapticSharpness,
                parameter_value: haptic_sharpness,
            },
        ];
        self.add_event("HapticTransient", time, parameters, None, None);
    }

    pub fn add_haptic_continuous_event(&mut self, time: f32, event_duration: f32, haptic_intensity: f32, haptic_sharpness: f32) {
        let parameters = vec![
            Parameter {
                parameter_id: ParamID::HapticIntensity,
                parameter_value: haptic_intensity,
            },
            Parameter {
                parameter_id: ParamID::HapticSharpness,
                parameter_value: haptic_sharpness,
            },
        ];
        self.add_event("HapticContinuous", time, parameters, Some(event_duration), None);
    }

    pub fn add_audio_custom_event(&mut self, time: f32, wav_filepath: &str, volume: f32) {
        let parameters = vec![
            Parameter {
                parameter_id: ParamID::AudioVolume,
                parameter_value: volume,
            },
        ];
        self.add_event("AudioCustom", time, parameters, None, Some(wav_filepath.to_string()));
    }

    pub fn add_parameter_curve(&mut self, parameter_id: CurveParamID, start_time: f32, control_points: Vec<HapticCurve>) {
        self.pattern.push(EventPattern {
            event: None,
            parameter_curve: Some(ParameterCurve {
                parameter_id,
                time: start_time,
                parameter_curve_control_points: control_points,
            }),
        });
    }

    pub fn create_curve(start_time: f32, end_time: f32, start_value: f32, end_value: f32, total: usize) -> Vec<HapticCurve> {
        let timediff = end_time - start_time;
        let valuediff = end_value - start_value;
        let timestep = timediff / total as f32;
        let valuestep = valuediff / total as f32;

        (1..=total).map(|i| {
            HapticCurve {
                time: round3(start_time + timestep * i as f32),
                parameter_value: round3(start_value + valuestep * i as f32),
            }
        }).collect()
    }

    pub fn export(&self, filename: impl Into<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
        let data = serde_json::to_string(&self)?;  // Minified JSON
        fs::write(filename.into(), data)?;
        Ok(())
    }

    pub fn export_pretty(&self, filename: impl Into<PathBuf>) -> Result<(), Box<dyn std::error::Error>>{
        let data = serde_json::to_string_pretty(&self)?;  // Minified JSON
        fs::write(filename.into(), data)?;
        Ok(())

    }

    pub fn import(filename: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data = fs::read_to_string(filename)?;
        let ahap: AHAP = serde_json::from_str(&data)?;
        Ok(ahap)
    }

    pub fn freq(n: u32, normalize: bool) -> Result<f32, String> {
        let mut n = n;
        if normalize {
            if n > 230 {
                n = 230;
            } else if n < 80 {
                n = 80;
            }
        }
        if n < 80 || n > 230 {
            return Err(format!("Incorrect frequency. Frequency must be between 80 and 230, but it is {}", n));
        }
        let r = (f32::ln(n as f32) - f32::ln(80.0)) / (f32::ln(230.0) - f32::ln(80.0));
        if r < 0.0 || r > 1.0 {
            return Err("The calculated normalized frequency is out of range. Result must be between 0 and 1.".to_string());
        }
        Ok(r)
    }
}
