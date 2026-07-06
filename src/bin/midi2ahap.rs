use ahap_rs::{freq_to_sharpness, Ahap, Continuous, Curve, Transient, CURVE_HAPTIC_INTENSITY};
use clap::Parser;
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use std::collections::HashMap;
use std::path::Path;
use std::process;

fn midi_note_to_freq(note: u8) -> f64 {
    440.0 * 2f64.powf((note as f64 - 69.0) / 12.0)
}

/// The Taptic Engine's continuous events only track frequency down to ~80 Hz;
/// below that a single tone doesn't read as a pitch anymore. So for low
/// notes, shift up by octaves until the root clears the floor, then add a
/// fourth below it as a second simultaneous note - e.g. C2 becomes C3+G2. Two
/// notes a fourth apart perceptually still reads as "that low note" much
/// better than one out-of-range tone.
fn notes_for_low_pitch(note: u8, floor_hz: f64) -> Vec<u8> {
    let mut root = note;
    while midi_note_to_freq(root) < floor_hz {
        match root.checked_add(12) {
            Some(next) => root = next,
            None => break, // already at the top of the MIDI range
        }
    }
    if root == note {
        vec![note]
    } else {
        vec![root, root - 5]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum HapticKind {
    Transient,
    Thump,
    Ringing,
}

#[derive(Debug, Clone, Copy)]
struct DrumMapping {
    kind: HapticKind,
    intensity: f64,
    sharpness: f64,
    duration: f64,
}

fn drum_mapping(note: u8) -> Option<DrumMapping> {
    use HapticKind::*;
    let m = |kind, intensity, sharpness, duration| Some(DrumMapping { kind, intensity, sharpness, duration });
    match note {
        35 | 36 => m(Thump, 1.0, 0.15, 0.09),                 // bass drums
        38 => m(Transient, 0.95, 0.85, 0.0),                  // acoustic snare
        40 => m(Transient, 0.9, 0.9, 0.0),                    // electric snare
        41 => m(Thump, 0.85, 0.30, 0.07),                     // low floor tom
        43 => m(Thump, 0.85, 0.35, 0.065),                    // high floor tom
        45 => m(Thump, 0.85, 0.40, 0.06),                     // low tom
        47 => m(Thump, 0.85, 0.45, 0.055),                    // low-mid tom
        48 => m(Thump, 0.85, 0.50, 0.05),                     // hi-mid tom
        50 => m(Thump, 0.85, 0.55, 0.045),                    // high tom
        42 => m(Transient, 0.5, 1.0, 0.0),                    // closed hi-hat
        44 => m(Transient, 0.5, 0.95, 0.0),                   // pedal hi-hat
        46 => m(Ringing, 0.6, 0.9, 0.25),                     // open hi-hat
        49 => m(Ringing, 0.9, 0.85, 0.6),                     // crash 1
        51 => m(Ringing, 0.6, 0.75, 0.35),                    // ride 1
        52 => m(Ringing, 0.85, 0.8, 0.55),                    // chinese cymbal
        53 => m(Transient, 0.65, 0.7, 0.0),                   // ride bell
        55 => m(Ringing, 0.75, 0.9, 0.3),                     // splash
        57 => m(Ringing, 0.9, 0.85, 0.6),                     // crash 2
        59 => m(Ringing, 0.6, 0.75, 0.35),                    // ride 2
        37 => m(Transient, 0.7, 0.95, 0.0),                   // side stick
        39 => m(Transient, 0.75, 0.8, 0.0),                   // hand clap
        54 => m(Ringing, 0.6, 0.85, 0.15),                    // tambourine
        56 => m(Transient, 0.7, 0.7, 0.0),                    // cowbell
        58 => m(Ringing, 0.65, 0.75, 0.3),                    // vibraslap
        60 => m(Thump, 0.75, 0.6, 0.04),                      // hi bongo
        61 => m(Thump, 0.75, 0.5, 0.05),                      // low bongo
        62 => m(Transient, 0.75, 0.65, 0.0),                  // mute hi conga
        63 => m(Thump, 0.75, 0.6, 0.05),                      // open hi conga
        64 => m(Thump, 0.75, 0.55, 0.06),                     // low conga
        65 => m(Thump, 0.8, 0.7, 0.04),                       // high timbale
        66 => m(Thump, 0.8, 0.65, 0.05),                      // low timbale
        67 => m(Transient, 0.7, 0.8, 0.0),                    // high agogo
        68 => m(Transient, 0.7, 0.75, 0.0),                   // low agogo
        69 => m(Transient, 0.55, 0.7, 0.0),                   // cabasa
        70 => m(Transient, 0.5, 0.85, 0.0),                   // maracas
        71 => m(Transient, 0.6, 0.9, 0.0),                    // short whistle
        72 => m(Ringing, 0.6, 0.85, 0.2),                     // long whistle
        73 => m(Transient, 0.65, 0.75, 0.0),                  // short guiro
        74 => m(Ringing, 0.65, 0.7, 0.15),                    // long guiro
        75 => m(Transient, 0.7, 0.95, 0.0),                   // claves
        76 => m(Transient, 0.7, 0.8, 0.0),                    // hi wood block
        77 => m(Transient, 0.7, 0.75, 0.0),                   // low wood block
        78 => m(Transient, 0.65, 0.7, 0.0),                   // mute cuica
        79 => m(Thump, 0.65, 0.65, 0.06),                     // open cuica
        80 => m(Transient, 0.55, 0.9, 0.0),                   // mute triangle
        81 => m(Ringing, 0.55, 0.95, 0.3),                    // open triangle
        _ => None,
    }
}

/// Renders one drum hit according to its instrument kind. This is the crux of
/// "realistic" drums: a kick/tom gets a short felt punch (Continuous + decay
/// envelope), a cymbal/open hi-hat gets a long Continuous event with a fading
/// intensity curve, and only snares/sticks/etc stay a flat instantaneous Transient.
fn add_drum_hit(ahap: &mut Ahap, t: f64, map: DrumMapping, intensity: f64) {
    match map.kind {
        HapticKind::Thump => {
            let decay = map.duration * 0.6;
            let release = map.duration * 0.4;
            let event = Continuous::at(t, map.duration)
                .intensity(intensity)
                .sharpness(map.sharpness)
                .attack(0.0)
                .decay(decay)
                .release(release)
                .build();
            ahap.add_event(event);
        }
        HapticKind::Ringing => {
            let event = Continuous::at(t, map.duration).intensity(intensity).sharpness(map.sharpness).build();
            ahap.add_event(event);

            // HapticIntensityControl multiplies the event's base HapticIntensity
            // (output = intensity * curve), so this ramps 1.0 -> 0.0, not
            // intensity -> 0, and starts with an explicit anchor at relative
            // time 0 so the ring starts at full strength instead of holding at
            // the first generated point's value.
            let curve = Curve::new(CURVE_HAPTIC_INTENSITY, t)
                .anchor(0.0, 1.0)
                .ease_in_out_to((0.0, 1.0), (map.duration, 0.0), 6)
                .build();
            ahap.add_parameter_curve(curve);
        }
        HapticKind::Transient => {
            let event = Transient::at(t).intensity(intensity).sharpness(map.sharpness).build();
            ahap.add_event(event);
        }
    }
}

const DRUM_CHANNEL: u8 = 9; // GM channel 10, 0-indexed

/// Convert a MIDI file to an AHAP haptic pattern, with realistic
/// per-instrument drum rendering on the GM drum channel (channel 10).
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Input .mid file
    input: String,

    /// Output .ahap file (default: <input>.ahap next to the input)
    output: Option<String>,

    /// Treat channel 10 as regular melodic notes instead of GM drums
    #[arg(long)]
    no_drums: bool,
}

fn main() {
    let cli = Cli::parse();
    let input = &cli.input;
    let no_drums = cli.no_drums;
    let output = cli.output.unwrap_or_else(|| {
        let stem = Path::new(input).file_stem().and_then(|s| s.to_str()).unwrap_or("output");
        let parent = Path::new(input).parent().unwrap_or_else(|| Path::new(""));
        parent.join(format!("{stem}.ahap")).to_string_lossy().into_owned()
    });

    let data = std::fs::read(input).unwrap_or_else(|e| {
        eprintln!("Failed to read MIDI file: {e}");
        process::exit(1);
    });
    let smf = Smf::parse(&data).unwrap_or_else(|e| {
        eprintln!("Failed to parse MIDI file: {e}");
        process::exit(1);
    });

    let ticks_per_quarter: f64 = match smf.header.timing {
        midly::Timing::Metrical(t) => u16::from(t) as f64,
        midly::Timing::Timecode(fps, subframe) => (fps.as_f32() as f64) * (subframe as f64), // approximation
    };

    let mut ahap = Ahap::new(
        format!("midi file {}", Path::new(input).file_name().unwrap_or_default().to_string_lossy()),
        "midi to haptic generator (rust)",
    );

    struct NoteInfo {
        start_time: f64,
        velocity: u8,
    }

    let mut drum_count = 0u32;
    let mut unknown_drum_count = 0u32;
    let mut melodic_count = 0u32;

    for track in &smf.tracks {
        let mut current_time = 0.0f64;
        let mut micros_per_quarter = 500_000.0f64; // 120 BPM default
        let mut note_state: HashMap<u8, NoteInfo> = HashMap::new();

        for event in track {
            current_time += event.delta.as_int() as f64 / ticks_per_quarter * (micros_per_quarter / 1_000_000.0);

            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    micros_per_quarter = t.as_int() as f64;
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            let key = key.as_int();
                            let velocity = vel.as_int();
                            if !no_drums && channel == DRUM_CHANNEL {
                                let velocity_scale = velocity as f64 / 127.0;
                                if let Some(map) = drum_mapping(key) {
                                    add_drum_hit(&mut ahap, current_time, map, map.intensity * velocity_scale);
                                    drum_count += 1;
                                } else {
                                    let event = Transient::at(current_time).intensity(velocity_scale).sharpness(0.7).build();
                                    ahap.add_event(event);
                                    drum_count += 1;
                                    unknown_drum_count += 1;
                                }
                            } else {
                                note_state.insert(key, NoteInfo { start_time: current_time, velocity });
                            }
                        }
                        // A NoteOn with velocity 0 is a NoteOff per the MIDI spec.
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            let key = key.as_int();
                            if !(!no_drums && channel == DRUM_CHANNEL) {
                                if let Some(info) = note_state.remove(&key) {
                                    let duration = current_time - info.start_time;
                                    if duration > 0.0 {
                                        let intensity = info.velocity as f64 / 127.0;
                                        for haptic_note in notes_for_low_pitch(key, 80.0) {
                                            let freq = midi_note_to_freq(haptic_note);
                                            let sharpness = freq_to_sharpness(freq, true).unwrap_or(0.5);
                                            let event = Continuous::at(info.start_time, duration)
                                                .intensity(intensity)
                                                .sharpness(sharpness)
                                                .build();
                                            ahap.add_event(event);
                                        }
                                        melodic_count += 1;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    if let Err(e) = ahap.export(&output, false) {
        eprintln!("Failed to export AHAP: {e}");
        process::exit(1);
    }

    println!("Successfully created {output}");
    println!("Conversion statistics:");
    println!("  Drum events: {drum_count}");
    if unknown_drum_count > 0 {
        println!("    (including {unknown_drum_count} unmapped drum notes)");
    }
    println!("  Melodic events (continuous): {melodic_count}");
    println!("  Total haptic events: {}", drum_count + melodic_count);
}

#[cfg(test)]
mod low_pitch_tests {
    use super::*;

    #[test]
    fn low_note_splits_into_root_and_fourth() {
        assert_eq!(notes_for_low_pitch(36, 80.0), vec![48, 43]); // C2 -> C3+G2
        assert_eq!(notes_for_low_pitch(60, 80.0), vec![60]);     // C4 stays single
    }
}
