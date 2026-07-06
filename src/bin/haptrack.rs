use ahap_rs::{Builder, CURVE_HAPTIC_SHARPNESS};
use clap::Parser;
use std::fs;
use std::process;

#[derive(Debug, Clone)]
struct HapticDefinition {
    intensity: f64,
    sharpness: f64,
    kind: EventKind,
    duration: f64, // continuous only
    curve: Option<(f64, f64, f64)>, // (start_sharp, end_sharp, duration)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum EventKind {
    Transient,
    Continuous,
}

impl Default for HapticDefinition {
    fn default() -> Self {
        Self { intensity: 0.8, sharpness: 0.5, kind: EventKind::Transient, duration: 0.05, curve: None }
    }
}

struct Haptrack {
    definitions: [Option<HapticDefinition>; 128],
    bpm: f64,
    numerator: u32,
    denominator: u32,
    builder: Option<Builder>,
}

impl Haptrack {
    fn new() -> Self {
        Self { definitions: std::array::from_fn(|_| None), bpm: 120.0, numerator: 4, denominator: 4, builder: None }
    }

    fn parse_file(&mut self, path: &str) -> Result<(), String> {
        let contents = fs::read_to_string(path).map_err(|e| e.to_string())?;
        let mut in_definitions = true;

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if line.eq_ignore_ascii_case("begin") {
                in_definitions = false;
                self.builder = Some(
                    Builder::new("Haptrack Pattern", "Haptrack DSL (Rust)")
                        .with_bpm(self.bpm)
                        .with_time_signature(self.numerator, self.denominator),
                );
                continue;
            }
            if in_definitions {
                self.parse_definition_line(line)?;
            } else {
                if line.to_ascii_lowercase().starts_with("track") {
                    continue;
                }
                self.parse_track(line)?;
            }
        }
        Ok(())
    }

    fn parse_definition_line(&mut self, line: &str) -> Result<(), String> {
        let Some((key, value)) = line.split_once('=') else { return Ok(()) };
        let key = key.trim();
        let value = value.trim();

        match key {
            "bpm" => {
                self.bpm = value.parse().map_err(|_| format!("invalid BPM: {value}"))?;
            }
            "time" => {
                let (num, den) = value.split_once('/').ok_or_else(|| format!("invalid time signature: {value}"))?;
                self.numerator = num.trim().parse().map_err(|_| format!("invalid time numerator: {num}"))?;
                self.denominator = den.trim().parse().map_err(|_| format!("invalid time denominator: {den}"))?;
            }
            letter if letter.chars().count() == 1 => {
                let ch = letter.chars().next().unwrap();
                if (ch as u32) < 128 {
                    self.definitions[ch as usize] = Some(parse_haptic_definition(value)?);
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn parse_track(&mut self, pattern: &str) -> Result<(), String> {
        let builder = self.builder.as_mut().ok_or("no builder initialized (missing 'begin'?)")?;
        let chars: Vec<char> = pattern.chars().collect();
        let mut i = 0usize;
        let mut current_beat = 0.0f64;

        while i < chars.len() {
            let c = chars[i];
            if c == '-' {
                i += 1;
                let duration = if i < chars.len() && chars[i].is_ascii_digit() {
                    parse_note_duration(&chars, &mut i)
                } else {
                    8
                };
                current_beat += beat_duration(duration, self.denominator);
                continue;
            }

            if (c as u32) < 128 {
                if let Some(def) = self.definitions[c as usize].clone() {
                    i += 1;
                    let duration = if i < chars.len() && chars[i].is_ascii_digit() {
                        parse_note_duration(&chars, &mut i)
                    } else {
                        8
                    };

                    let beats_per_bar = builder.beats_per_bar();
                    let bar = (current_beat as i64) / beats_per_bar as i64;
                    let beat = (current_beat as i64) % beats_per_bar as i64;
                    let time = builder.at(bar, beat);

                    match def.kind {
                        EventKind::Continuous => {
                            builder.add_continuous(time, def.duration, def.intensity, def.sharpness);
                        }
                        EventKind::Transient => {
                            builder.add_transient(time, def.intensity, def.sharpness);
                        }
                    }

                    if let Some((start, end, curve_dur)) = def.curve {
                        builder.add_curve(CURVE_HAPTIC_SHARPNESS, time, vec![(0.0, start), (curve_dur, end)], 5);
                    }

                    current_beat += beat_duration(duration, self.denominator);
                    continue;
                }
            }
            i += 1; // unknown character, skip
        }
        Ok(())
    }
}

/// Parses one haptic letter definition value. Supports two syntaxes:
///
/// - New: `name: type; intensity=0.9, sharpness_curve=0.5-0.2; duration=0.1`
/// - Old/CSV (what the shipped example .hap files actually use):
///   `name, intensity, sharpness[, curve_direction, duration_ms]`
///
/// The Go version only implemented the new syntax, so every existing
/// `.hap` example file (which uses the CSV form) silently parsed as
/// `Name = "snare, 1.0, 0.9, down, 60"` with all other fields left at
/// their defaults - the custom intensity/sharpness/curve settings were
/// quietly discarded. Supporting both here fixes that for real files.
fn parse_haptic_definition(value: &str) -> Result<HapticDefinition, String> {
    if value.contains(':') {
        return parse_new_syntax(value);
    }
    parse_csv_syntax(value)
}

fn parse_new_syntax(value: &str) -> Result<HapticDefinition, String> {
    let mut def = HapticDefinition::default();
    let (_, params_part) = value.split_once(':').unwrap();

    for group in params_part.split(';') {
        let group = group.trim();
        if group.is_empty() {
            continue;
        }
        if !group.contains('=') && !group.contains(',') {
            match group.to_ascii_lowercase().as_str() {
                "continuous" => def.kind = EventKind::Continuous,
                "transient" => def.kind = EventKind::Transient,
                _ => {}
            }
            continue;
        }

        let mut curve_start_end: Option<(f64, f64)> = None;
        let mut curve_dur = 0.06;

        for param in group.split(',') {
            let param = param.trim();
            if param.is_empty() {
                continue;
            }
            let Some((key, val)) = param.split_once('=') else { continue };
            let key = key.trim();
            let val = val.trim();
            match key {
                "intensity" => def.intensity = val.parse().map_err(|_| format!("invalid intensity: {val}"))?,
                "sharpness" => def.sharpness = val.parse().map_err(|_| format!("invalid sharpness: {val}"))?,
                "duration" => def.duration = val.parse().map_err(|_| format!("invalid duration: {val}"))?,
                "curve_duration" => curve_dur = val.parse().map_err(|_| format!("invalid curve_duration: {val}"))?,
                "sharpness_curve" => {
                    if let Some((s, e)) = val.split_once('-') {
                        let s: f64 = s.trim().parse().map_err(|_| format!("invalid curve start: {s}"))?;
                        let e: f64 = e.trim().parse().map_err(|_| format!("invalid curve end: {e}"))?;
                        curve_start_end = Some((s, e));
                    }
                }
                _ => {}
            }
        }
        if let Some((s, e)) = curve_start_end {
            def.curve = Some((s, e, curve_dur));
        }
    }
    Ok(def)
}

fn parse_csv_syntax(value: &str) -> Result<HapticDefinition, String> {
    let mut def = HapticDefinition::default();
    let fields: Vec<&str> = value.split(',').map(str::trim).collect();
    // fields[0] = name (ignored, informational only)
    if fields.len() >= 2 {
        def.intensity = fields[1].parse().map_err(|_| format!("invalid intensity: {}", fields[1]))?;
    }
    if fields.len() >= 3 {
        def.sharpness = fields[2].parse().map_err(|_| format!("invalid sharpness: {}", fields[2]))?;
    }
    if fields.len() >= 4 {
        // curve direction: "down" ramps sharpness -> 0, "up" ramps 0 -> sharpness
        let duration_ms: f64 = if fields.len() >= 5 {
            fields[4].parse().unwrap_or(60.0)
        } else {
            60.0
        };
        let duration_s = duration_ms / 1000.0;
        def.curve = match fields[3].to_ascii_lowercase().as_str() {
            "down" => Some((def.sharpness, 0.0, duration_s)),
            "up" => Some((0.0, def.sharpness, duration_s)),
            _ => None,
        };
    }
    Ok(def)
}

fn parse_note_duration(chars: &[char], i: &mut usize) -> u32 {
    let start = *i;
    while *i < chars.len() && chars[*i].is_ascii_digit() {
        *i += 1;
    }
    chars[start..*i].iter().collect::<String>().parse().unwrap_or(8)
}

/// 1=whole, 2=half, 4=quarter, 8=eighth, 16=sixteenth, relative to the
/// time signature's denominator (which note value gets one beat).
fn beat_duration(note_value: u32, denominator: u32) -> f64 {
    (4.0 / note_value as f64) * (denominator as f64 / 4.0)
}

/// Compiles a haptrack DSL file (drum-style patterns written with letters
/// and note durations) into an AHAP haptic pattern.
///
/// File format:
///   bpm = 120
///   time = 4/4
///   s = snare, 1.0, 0.9, down, 60
///   k = kick, 1.0, 0.2
///   h = hihat, 0.6, 1.0
///
///   begin
///   track1
///   k8k8s8k8k8k8s8k8
///   track2
///   h8h8h8h8h8h8h8h8
///
/// Note durations: 1=whole, 2=half, 4=quarter, 8=eighth, 16=sixteenth.
/// Rest: - (dash), e.g. s8-8 means a snare eighth note then an eighth rest.
#[derive(Parser, Debug)]
#[command(version, about, long_about = None, verbatim_doc_comment)]
struct Cli {
    /// Input .hap file
    input: String,

    /// Output .ahap file
    #[arg(default_value = "output.ahap")]
    output: String,
}

fn main() {
    let cli = Cli::parse();
    let input = &cli.input;
    let output = cli.output;

    let mut parser = Haptrack::new();
    println!("Parsing haptrack file: {input}");
    if let Err(e) = parser.parse_file(input) {
        eprintln!("Error parsing file: {e}");
        process::exit(1);
    }

    let defined = parser.definitions.iter().filter(|d| d.is_some()).count();
    println!("Found {defined} haptic definitions");
    println!("BPM: {:.0}, Time Signature: {}/{}", parser.bpm, parser.numerator, parser.denominator);

    let Some(builder) = parser.builder else {
        eprintln!("No tracks found in file (missing 'begin' marker?)");
        process::exit(1);
    };

    if let Err(e) = builder.build().export(&output, true) {
        eprintln!("Error exporting AHAP: {e}");
        process::exit(1);
    }
    println!("Successfully created {output}");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_syntax_parses_intensity_and_sharpness() {
        let def = parse_haptic_definition("snare, 1.0, 0.9, down, 60").unwrap();
        assert_eq!(def.intensity, 1.0);
        assert_eq!(def.sharpness, 0.9);
        assert_eq!(def.kind, EventKind::Transient);
        let (start, end, dur) = def.curve.expect("expected a curve for 'down'");
        assert_eq!(start, 0.9);
        assert_eq!(end, 0.0);
        assert!((dur - 0.06).abs() < 1e-9);
    }

    #[test]
    fn csv_syntax_without_curve() {
        let def = parse_haptic_definition("kick, 1.0, 0.2").unwrap();
        assert_eq!(def.intensity, 1.0);
        assert_eq!(def.sharpness, 0.2);
        assert!(def.curve.is_none());
    }

    #[test]
    fn new_syntax_parses_continuous_with_curve() {
        let def = parse_haptic_definition("kick: continuous; intensity=0.9, sharpness_curve=0.5-0.2; duration=0.1").unwrap();
        assert_eq!(def.kind, EventKind::Continuous);
        assert_eq!(def.intensity, 0.9);
        assert_eq!(def.duration, 0.1);
        let (start, end, _) = def.curve.expect("expected a curve");
        assert_eq!(start, 0.5);
        assert_eq!(end, 0.2);
    }

    #[test]
    fn beat_duration_math() {
        // In 4/4, a quarter note (4) is exactly one beat.
        assert!((beat_duration(4, 4) - 1.0).abs() < 1e-9);
        // In 4/4, an eighth note (8) is half a beat.
        assert!((beat_duration(8, 4) - 0.5).abs() < 1e-9);
    }
}
