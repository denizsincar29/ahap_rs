//! # midi2ahap
//!
//! Converts a standard MIDI (`.mid`) file into an AHAP haptic pattern.
//!
//! How it works: every track is walked event by event, converting delta
//! ticks to seconds using the tempo active at that point (tempo changes
//! mid-track are handled correctly, not just at the start). Two kinds of
//! MIDI events become haptics:
//!
//! - **Channel 10 (GM drum channel), unless `--no-drums`:** each drum note
//!   is looked up in [`DRUM_MAPPINGS`] and rendered by [`add_drum_hit`]
//!   according to its [`HapticKind`] - a crisp instantaneous `Transient` for
//!   snares/sticks, a short felt "punch" (`Continuous` + decay/release
//!   envelope) for kicks/toms, or a long ringing `Continuous` event with a
//!   fading intensity curve for cymbals/open hi-hat. An unmapped drum note
//!   still gets a generic transient so nothing is silently dropped.
//! - **Everything else (melodic notes):** each note-on/note-off pair becomes
//!   one `Continuous` event spanning its duration, with pitch mapped to
//!   [`ahap_rs::freq_to_sharpness`]. Notes below the Taptic Engine's ~80 Hz
//!   floor are split into two simultaneous notes (see
//!   [`notes_for_low_pitch`]) since a single out-of-range tone doesn't read
//!   as a pitch at all.
//!
//! **Live envelope/brightness control from the MIDI file itself:** standard
//! General MIDI 2 "Sound Controller" CC messages - CC 73 (Attack Time), CC
//! 72 (Release Time), CC 75 (Decay Time), CC 74 (Brightness) - override the
//! computed attack/decay/release and nudge sharpness for every event from
//! that point on, across every track. CC 72/73/75 are *relative*: a CC
//! value maps to a fraction (0.0-1.0) of each event's own duration, not an
//! absolute number of seconds, so a CC of 100 never smears a 0.15s note
//! into a longer hum than the note itself. See [`GlobalControl`] for the
//! exact mapping and an important ordering caveat.
//!
//! Usage: `midi2ahap <input.mid> [output.ahap] [--no-drums] [--drums-as-melody] [--debug-channels]`

use ahap_rs::{freq_to_sharpness, Ahap, Continuous, Curve, Transient, CURVE_HAPTIC_INTENSITY};
use clap::Parser;
use midly::{MetaMessage, MidiMessage, Smf, TrackEventKind};
use std::collections::HashMap;
use std::path::Path;
use std::process;

/// Standard equal-temperament MIDI note number to frequency in Hz (A4 = note 69 = 440 Hz).
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

/// How a drum hit should be rendered as haptics - see the module doc for the
/// reasoning behind splitting drums into these three shapes.
#[derive(Debug, Clone, Copy, PartialEq)]
enum HapticKind {
    /// Instantaneous snap: snares, sticks, claves, closed/pedal hi-hat.
    Transient,
    /// Short felt punch with a bit of body: kicks, toms, congas, bongos.
    Thump,
    /// Long decaying tail: cymbals, open hi-hat, tambourine, triangle.
    Ringing,
}

/// Haptic characteristics for one General MIDI percussion note.
#[derive(Debug, Clone, Copy)]
struct DrumMapping {
    kind: HapticKind,
    intensity: f64,
    sharpness: f64,
    /// Event duration in seconds. Only meaningful for `Thump`/`Ringing`
    /// (rendered as `Continuous` events); ignored for `Transient`.
    duration: f64,
}

/// Looks up the haptic rendering for a General MIDI percussion note (35-81).
/// Returns `None` for notes outside the standard GM drum map.
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

/// Live-updatable global envelope/brightness state, driven by standard
/// General MIDI 2 "Sound Controller" CC messages found in the file itself:
/// CC 73 (Attack Time), CC 72 (Release Time), CC 75 (Decay Time), CC 74
/// (Brightness). These are official GM2 CCs, not invented ones - any DAW
/// can already draw automation for them. `None` means "no override seen
/// yet, keep using each drum kind's own computed default"; once a CC is
/// seen it overrides that field for every event from then on, across every
/// track (this is *global*, not per-channel), until a new value arrives.
///
/// Attack/decay/release are stored as **fractions of each event's own
/// duration** (0.0-1.0), not absolute seconds. A CC of 127 means "this
/// whole event is envelope", a CC of 100 means ~0.79 of the event's
/// duration, etc. - the same relative scheme [`add_drum_hit`]'s `Thump`
/// case always used (`map.duration * 0.6/0.4`). Storing an absolute
/// second value here was the original bug: a fixed ~0.79s release from a
/// CC72=100 completely swallowed 0.15-0.36s note durations, smearing
/// distinct hits into one continuous hum. Resolving to seconds happens at
/// the point of use, via [`GlobalControl::attack_for`],
/// [`GlobalControl::decay_for`], and [`GlobalControl::release_for`], once
/// the actual event duration is known.
///
/// Caveat: tracks are processed one after another, not interleaved in true
/// chronological order, so a CC on one track only reliably affects events
/// on tracks processed *after* it. Put control CCs in track 0 (or a
/// dedicated first track) of a Type-1 file, or use a Type-0 file, to avoid
/// ordering surprises.
#[derive(Debug, Clone, Copy, Default)]
struct GlobalControl {
    /// Fraction (0.0-1.0) of an event's duration to use as attack time.
    attack: Option<f64>,
    /// Fraction (0.0-1.0) of an event's duration to use as decay time.
    decay: Option<f64>,
    /// Fraction (0.0-1.0) of an event's duration to use as release time.
    release: Option<f64>,
    /// Additive offset applied to computed sharpness, clamped to [0, 1] after.
    brightness_offset: f64,
}

const CC_RELEASE_TIME: u8 = 72;
const CC_ATTACK_TIME: u8 = 73;
const CC_BRIGHTNESS: u8 = 74;
const CC_DECAY_TIME: u8 = 75;

/// Reference duration used to resolve attack/decay/release fractions for
/// events that have no duration of their own (`Transient` hits are
/// instantaneous). Keeps CC-driven envelope shaping meaningful even there,
/// without ever reintroducing an absolute multi-hundred-ms smear.
const TRANSIENT_REFERENCE_SECONDS: f64 = 0.1;
/// Max +/- sharpness offset a CC value of 0/127 maps to (64 = no offset).
const MAX_BRIGHTNESS_OFFSET: f64 = 0.3;

impl GlobalControl {
    /// Updates state from one CC message. Returns true if it was one of ours.
    fn apply_cc(&mut self, controller: u8, value: u8) -> bool {
        match controller {
            CC_ATTACK_TIME => {
                self.attack = Some(value as f64 / 127.0);
                true
            }
            CC_RELEASE_TIME => {
                self.release = Some(value as f64 / 127.0);
                true
            }
            CC_DECAY_TIME => {
                self.decay = Some(value as f64 / 127.0);
                true
            }
            CC_BRIGHTNESS => {
                self.brightness_offset = (value as f64 - 64.0) / 63.0 * MAX_BRIGHTNESS_OFFSET;
                true
            }
            _ => false,
        }
    }

    fn adjust_sharpness(&self, sharpness: f64) -> f64 {
        (sharpness + self.brightness_offset).clamp(0.0, 1.0)
    }

    /// Resolves the attack-time CC override to seconds, relative to `duration`.
    fn attack_for(&self, duration: f64) -> Option<f64> {
        self.attack.map(|frac| frac * duration)
    }

    /// Resolves the decay-time CC override to seconds, relative to `duration`.
    fn decay_for(&self, duration: f64) -> Option<f64> {
        self.decay.map(|frac| frac * duration)
    }

    /// Resolves the release-time CC override to seconds, relative to `duration`.
    fn release_for(&self, duration: f64) -> Option<f64> {
        self.release.map(|frac| frac * duration)
    }
}

/// Renders one drum hit according to its instrument kind. This is the crux of
/// "realistic" drums: a kick/tom gets a short felt punch (Continuous + decay
/// envelope), a cymbal/open hi-hat gets a long Continuous event with a fading
/// intensity curve, and only snares/sticks/etc stay a flat instantaneous Transient.
/// `control`'s attack/decay/release/brightness, if set via CC, override the
/// per-kind computed defaults below.
fn add_drum_hit(ahap: &mut Ahap, t: f64, map: DrumMapping, intensity: f64, control: &GlobalControl) {
    let sharpness = control.adjust_sharpness(map.sharpness);
    match map.kind {
        HapticKind::Thump => {
            let attack = control.attack_for(map.duration).unwrap_or(0.0);
            let decay = control.decay_for(map.duration).unwrap_or(map.duration * 0.6);
            let release = control.release_for(map.duration).unwrap_or(map.duration * 0.4);
            let event = Continuous::at(t, map.duration)
                .intensity(intensity)
                .sharpness(sharpness)
                .attack(attack)
                .decay(decay)
                .release(release)
                .build();
            ahap.add_event(event);
        }
        HapticKind::Ringing => {
            let mut builder = Continuous::at(t, map.duration).intensity(intensity).sharpness(sharpness);
            if let Some(a) = control.attack_for(map.duration) {
                builder = builder.attack(a);
            }
            if let Some(r) = control.release_for(map.duration) {
                builder = builder.release(r);
            }
            ahap.add_event(builder.build());

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
            let mut builder = Transient::at(t).intensity(intensity).sharpness(sharpness);
            if let Some(a) = control.attack_for(TRANSIENT_REFERENCE_SECONDS) {
                builder = builder.attack(a);
            }
            if let Some(r) = control.release_for(TRANSIENT_REFERENCE_SECONDS) {
                builder = builder.release(r);
            }
            ahap.add_event(builder.build());
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

    /// Completely ignore channel 10 (GM drums) - no haptic events at all
    /// from that channel, only the melodic channels play
    #[arg(long, conflicts_with = "drums_as_melody")]
    no_drums: bool,

    /// Treat channel 10 as regular melodic notes instead of GM drums,
    /// rather than dropping it (see --no-drums to drop it instead)
    #[arg(long)]
    drums_as_melody: bool,

    /// Print how many note-on events came from each channel (only channels
    /// with at least one event are listed), to check what's actually in
    /// the file and whether --no-drums/--drums-as-melody are doing what
    /// you expect
    #[arg(long)]
    debug_channels: bool,
}

fn main() {
    let cli = Cli::parse();
    let input = &cli.input;
    let no_drums = cli.no_drums;
    let drums_as_melody = cli.drums_as_melody;
    let debug_channels = cli.debug_channels;
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
    let mut channel_counts: HashMap<u8, u32> = HashMap::new();
    // Shared across every track on purpose - see GlobalControl's doc comment
    // for the chronological-ordering caveat that comes with that.
    let mut control = GlobalControl::default();

    for track in &smf.tracks {
        let mut current_time = 0.0f64;
        let mut micros_per_quarter = 500_000.0f64; // 120 BPM default
        // Keyed by (channel, note), not just note: a MIDI file with several
        // channels active at once (very common - almost every real-world
        // song MIDI has one channel per instrument) can easily have two
        // channels holding the same pitch simultaneously. Keying by note
        // alone let a note-on on one channel clobber another channel's
        // still-open note of the same pitch, corrupting durations or
        // dropping notes outright.
        let mut note_state: HashMap<(u8, u8), NoteInfo> = HashMap::new();

        for event in track {
            current_time += event.delta.as_int() as f64 / ticks_per_quarter * (micros_per_quarter / 1_000_000.0);

            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => {
                    micros_per_quarter = t.as_int() as f64;
                }
                TrackEventKind::Midi { channel, message } => {
                    let channel = channel.as_int();
                    match message {
                        MidiMessage::Controller { controller, value } => {
                            control.apply_cc(controller.as_int(), value.as_int());
                        }
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            let key = key.as_int();
                            let velocity = vel.as_int();
                            *channel_counts.entry(channel).or_insert(0) += 1;
                            let is_drum_channel = channel == DRUM_CHANNEL;

                            if is_drum_channel && no_drums {
                                // Fully ignored: no event, no note_state entry.
                            } else if is_drum_channel && !drums_as_melody {
                                let velocity_scale = velocity as f64 / 127.0;
                                if let Some(map) = drum_mapping(key) {
                                    add_drum_hit(&mut ahap, current_time, map, map.intensity * velocity_scale, &control);
                                    drum_count += 1;
                                } else {
                                    let event = Transient::at(current_time).intensity(velocity_scale).sharpness(0.7).build();
                                    ahap.add_event(event);
                                    drum_count += 1;
                                    unknown_drum_count += 1;
                                }
                            } else {
                                // Either a melodic channel, or channel 10 with
                                // --drums-as-melody: treat as a regular note.
                                note_state.insert((channel, key), NoteInfo { start_time: current_time, velocity });
                            }
                        }
                        // A NoteOn with velocity 0 is a NoteOff per the MIDI spec.
                        MidiMessage::NoteOn { key, .. } | MidiMessage::NoteOff { key, .. } => {
                            let key = key.as_int();
                            let is_drum_channel = channel == DRUM_CHANNEL;
                            let treated_as_melodic = !is_drum_channel || drums_as_melody;
                            if treated_as_melodic {
                                if let Some(info) = note_state.remove(&(channel, key)) {
                                    let duration = current_time - info.start_time;
                                    if duration > 0.0 {
                                        let intensity = info.velocity as f64 / 127.0;
                                        for haptic_note in notes_for_low_pitch(key, 80.0) {
                                            let freq = midi_note_to_freq(haptic_note);
                                            let sharpness = control.adjust_sharpness(freq_to_sharpness(freq, true).unwrap_or(0.5));
                                            let mut builder = Continuous::at(info.start_time, duration)
                                                .intensity(intensity)
                                                .sharpness(sharpness);
                                            if let Some(a) = control.attack_for(duration) {
                                                builder = builder.attack(a);
                                            }
                                            if let Some(d) = control.decay_for(duration) {
                                                builder = builder.decay(d);
                                            }
                                            if let Some(r) = control.release_for(duration) {
                                                builder = builder.release(r);
                                            }
                                            ahap.add_event(builder.build());
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

    if debug_channels {
        println!("Note-on events per channel (1-indexed for readability):");
        let mut channels: Vec<_> = channel_counts.into_iter().filter(|&(_, n)| n > 0).collect();
        channels.sort_by_key(|&(ch, _)| ch);
        for (channel, count) in channels {
            let marker = if channel == DRUM_CHANNEL { " (GM drum channel)" } else { "" };
            println!("  channel {}: {count} events{marker}", channel + 1);
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

#[cfg(test)]
mod global_control_tests {
    use super::*;

    #[test]
    fn cc_values_map_to_fractions_and_offset() {
        let mut control = GlobalControl::default();
        assert!(control.apply_cc(CC_ATTACK_TIME, 127));
        assert!((control.attack.unwrap() - 1.0).abs() < 1e-9);

        assert!(control.apply_cc(CC_RELEASE_TIME, 0));
        assert!((control.release.unwrap() - 0.0).abs() < 1e-9);

        assert!(control.apply_cc(CC_DECAY_TIME, 64));
        assert!(control.decay.unwrap() > 0.49 && control.decay.unwrap() < 0.51);

        assert!(control.apply_cc(CC_BRIGHTNESS, 64)); // ~center, near-zero offset
        assert!(control.adjust_sharpness(0.5) > 0.45 && control.adjust_sharpness(0.5) < 0.55);

        assert!(!control.apply_cc(1, 100)); // unrelated CC (mod wheel) is ignored
    }

    #[test]
    fn cc_release_never_exceeds_short_note_duration() {
        // This is the exact bug scenario: CC72=100 arriving once at tick 0,
        // then short drum/melodic notes (0.15-0.36s) later in the file.
        let mut control = GlobalControl::default();
        control.apply_cc(CC_RELEASE_TIME, 100);

        let short_note_duration = 0.15;
        let release = control.release_for(short_note_duration).unwrap();
        assert!(release <= short_note_duration);
        assert!((release - short_note_duration * (100.0 / 127.0)).abs() < 1e-9);

        // A longer note gets a proportionally longer release, not the same
        // fixed absolute time as the short note.
        let long_note_duration = 1.2;
        let long_release = control.release_for(long_note_duration).unwrap();
        assert!((long_release - long_note_duration * (100.0 / 127.0)).abs() < 1e-9);
        assert!(long_release > release);
    }

    #[test]
    fn no_cc_seen_means_no_override() {
        let control = GlobalControl::default();
        assert!(control.attack.is_none());
        assert!(control.decay.is_none());
        assert!(control.release.is_none());
        assert_eq!(control.adjust_sharpness(0.42), 0.42);
    }
}
