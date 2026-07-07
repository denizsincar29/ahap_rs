//! # Haptic melody format (`.hmel`)
//!
//! A small text DSL for writing a haptic pattern the way you'd write a tune:
//! note letters and rests instead of raw seconds/timestamps. No time
//! signature - whitespace and newlines are purely cosmetic, only there to
//! let you visually group bars however you like.
//!
//! ```text
//! @name mario
//! @description super mario bros track
//! @tempo 200
//! @octave 3        # default octave
//! @accent-velocity +0.2   # can be with or without +; a negative value reverses accents
//! @melody
//! @f               # forte: dynamics level 8.0/10
//! EE-E-CE- !G---<G---
//! @drums
//! k-s-k-s- !k-s-k-!s-
//! ```
//!
//! ## Directives
//! All start with `@`, optionally followed by whitespace before the keyword
//! (`@tempo` and `@ tempo` both work). A `#` or `//` anywhere on the line
//! starts a comment running to end of line.
//!
//! - `@name <text>` / `@description <text>` - metadata only, copied into the
//!   exported AHAP's `Metadata`.
//! - `@tempo <bpm>` - beats (quarter notes) per minute. Default 120.
//! - `@octave <n>` - starting octave, scientific pitch notation (`C4` = middle
//!   C = MIDI 60). Default 4.
//! - `@duration <name-or-denominator>` - default note length when a note
//!   doesn't specify its own: `whole`/`1`, `half`/`2`, `quarter`/`4`,
//!   `eighth`/`8` (the built-in default), `sixteenth`/`16`,
//!   `thirtysecond`/`32`.
//! - `@duration-mode sticky|temporary` - whether a number written after a
//!   note (e.g. `F#4`) only overrides that one note (`temporary`, the
//!   default) or becomes the new default for every note after it
//!   (`sticky`), until changed again.
//! - `@accent-velocity <delta>` - intensity delta (on the final 0.0-1.0
//!   scale) applied to notes marked with `!`. Accepts a leading `+` or `-`;
//!   a negative value means accented notes get *quieter* instead of louder.
//!   Default `+0.15`.
//! - Dynamics markings, each sets the current intensity level on a 0-10
//!   scale (normalized to 0.0-1.0 for AHAP by dividing by 10):
//!   `@pp` = 2.0, `@p` = 4.0, `@mp` = 5.0, `@mf` = 6.0, `@f`/`@forte` = 8.0,
//!   `@ff` = 10.0. Default is `@mf` (6.0).
//! - `@melody` / `@drums` - switch the following body text into melody mode
//!   or drum mode. Can appear more than once to interleave sections; state
//!   like octave/tempo/dynamics carries over between them.
//!
//! ## Melody body
//! - `A`-`G` (case-insensitive) are notes. A `#` right after the letter
//!   makes it sharp (`F#`).
//! - `-` is a rest.
//! - `!` right before a note or rest is an accent (see `@accent-velocity`).
//! - `<` shifts the current octave down one, `>` shifts it up one. Shifts
//!   persist until the next shift or `@octave` directive.
//! - Digits right after a note/rest override its duration for just that
//!   symbol (or the running default, if `@duration-mode sticky`); see
//!   `@duration` for the accepted values.
//! - A computed frequency is clamped to the Taptic Engine's 80-230 Hz
//!   sharpness range (a warning is printed to stderr when that happens) -
//!   notes are never allowed to render outside of it.
//!
//! ## Drum body
//! Single letters, case-insensitive, no octave shifts:
//!
//! | Letter | Drum             | Haptic shape                        |
//! |--------|------------------|--------------------------------------|
//! | `k`    | kick             | short felt punch (`Continuous`)       |
//! | `t`    | tom              | short felt punch (`Continuous`)       |
//! | `s`    | snare            | crisp instantaneous (`Transient`)     |
//! | `h`    | closed hi-hat    | crisp instantaneous (`Transient`)     |
//! | `x`    | clap             | crisp instantaneous (`Transient`)     |
//! | `o`    | open hi-hat      | ringing tail (`Continuous` + curve)    |
//! | `c`    | crash cymbal     | ringing tail (`Continuous` + curve)    |
//! | `r`    | ride cymbal      | ringing tail (`Continuous` + curve)    |
//!
//! `-`, `!`, and duration digits work exactly like in the melody body.

use crate::{freq_to_sharpness, Ahap, Continuous, Curve, MusicalContext, Transient, CURVE_HAPTIC_INTENSITY};

/// One parsed dynamics marking, on a 0-10 scale (matches how musicians think
/// about `pp`/`f`/`ff`); normalized to 0.0-1.0 right before building an event.
fn dynamics_level(name: &str) -> Option<f64> {
    match name {
        "pp" => Some(2.0),
        "p" => Some(4.0),
        "mp" => Some(5.0),
        "mf" => Some(6.0),
        "f" | "forte" => Some(8.0),
        "ff" => Some(10.0),
        _ => None,
    }
}

/// Resolves a duration name or bare denominator (e.g. `"eighth"` or `"8"`)
/// to a denominator, where duration in beats is `4.0 / denominator`.
fn duration_denominator(name: &str) -> Option<u32> {
    match name {
        "whole" | "1" => Some(1),
        "half" | "2" => Some(2),
        "quarter" | "4" => Some(4),
        "eighth" | "8" => Some(8),
        "sixteenth" | "16" => Some(16),
        "thirtysecond" | "32" => Some(32),
        _ => None,
    }
}

/// Semitone offset from C for a note letter (A-G, case-insensitive).
fn note_semitone(letter: char) -> Option<i32> {
    match letter.to_ascii_uppercase() {
        'C' => Some(0),
        'D' => Some(2),
        'E' => Some(4),
        'F' => Some(5),
        'G' => Some(7),
        'A' => Some(9),
        'B' => Some(11),
        _ => None,
    }
}

/// Frequency in Hz for a note letter (+ optional sharp) at a given octave,
/// scientific pitch notation (`C4` = MIDI 60 = middle C, `A4` = 440 Hz).
fn note_freq(letter: char, sharp: bool, octave: i32) -> f64 {
    let semitone = note_semitone(letter).unwrap_or(0) + if sharp { 1 } else { 0 };
    let midi_number = (octave + 1) * 12 + semitone;
    440.0 * 2f64.powf((midi_number as f64 - 69.0) / 12.0)
}

/// Parsed representation of one drum letter's haptic shape.
#[derive(Clone, Copy)]
enum DrumShape {
    Punch { sharpness: f64 },
    Hit { sharpness: f64 },
    Ring { sharpness: f64 },
}

fn drum_shape(letter: char) -> Option<DrumShape> {
    match letter.to_ascii_lowercase() {
        'k' => Some(DrumShape::Punch { sharpness: 0.3 }),
        't' => Some(DrumShape::Punch { sharpness: 0.4 }),
        's' => Some(DrumShape::Hit { sharpness: 0.6 }),
        'h' => Some(DrumShape::Hit { sharpness: 0.75 }),
        'x' => Some(DrumShape::Hit { sharpness: 0.7 }),
        'o' => Some(DrumShape::Ring { sharpness: 0.65 }),
        'c' => Some(DrumShape::Ring { sharpness: 0.6 }),
        'r' => Some(DrumShape::Ring { sharpness: 0.55 }),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum DurationMode {
    Temporary,
    Sticky,
}

#[derive(Clone, Copy, PartialEq)]
enum Section {
    None,
    Melody,
    Drums,
}

/// Parses `.hmel` source text into an [`Ahap`] pattern.
pub fn parse_melody(source: &str) -> Result<Ahap, String> {
    let mut name = String::from("untitled");
    let mut description = String::from("haptic melody");
    let mut bpm = 120.0;
    let mut octave = 4;
    let mut default_denominator = 8; // eighth, per the format's built-in default
    let mut duration_mode = DurationMode::Temporary;
    let mut accent_delta = 0.15;
    let mut dynamics = 6.0; // mf
    let mut section = Section::None;
    let mut time = 0.0;
    let mut ahap: Option<Ahap> = None;

    for raw_line in source.lines() {
        // Strip comments: `//` always starts one; a bare `#` only starts one
        // when it's its own token (start of line or preceded by whitespace)
        // so it doesn't collide with `#` used as a sharp inside a note like
        // `F#4`, which is never preceded by whitespace.
        let slash_pos = raw_line.find("//");
        let mut hash_pos = None;
        let mut prev_is_space = true;
        for (i, ch) in raw_line.char_indices() {
            if ch == '#' && prev_is_space {
                hash_pos = Some(i);
                break;
            }
            prev_is_space = ch.is_whitespace();
        }
        let cut = match (hash_pos, slash_pos) {
            (Some(h), Some(s)) => h.min(s),
            (Some(h), None) => h,
            (None, Some(s)) => s,
            (None, None) => raw_line.len(),
        };
        let line = raw_line[..cut].trim();
        if line.is_empty() {
            continue;
        }

        if let Some(rest) = line.strip_prefix('@') {
            let rest = rest.trim_start();
            let (keyword, args) = match rest.split_once(char::is_whitespace) {
                Some((k, a)) => (k, a.trim()),
                None => (rest, ""),
            };
            match keyword {
                "name" => name = args.to_string(),
                "description" => description = args.to_string(),
                "tempo" => {
                    bpm = args.parse::<f64>().map_err(|_| format!("bad @tempo value: {args:?}"))?;
                }
                "octave" => {
                    octave = args.parse::<i32>().map_err(|_| format!("bad @octave value: {args:?}"))?;
                }
                "duration" => {
                    default_denominator =
                        duration_denominator(args).ok_or_else(|| format!("unknown @duration value: {args:?}"))?;
                }
                "duration-mode" => {
                    duration_mode = match args {
                        "sticky" | "persistent" => DurationMode::Sticky,
                        "temporary" | "oneshot" | "once" => DurationMode::Temporary,
                        _ => return Err(format!("unknown @duration-mode value: {args:?}")),
                    };
                }
                "accent-velocity" => {
                    accent_delta =
                        args.parse::<f64>().map_err(|_| format!("bad @accent-velocity value: {args:?}"))?;
                }
                "melody" => section = Section::Melody,
                "drums" => section = Section::Drums,
                other => {
                    if let Some(level) = dynamics_level(other) {
                        dynamics = level;
                    } else {
                        return Err(format!("unknown directive: @{other}"));
                    }
                }
            }
            continue;
        }

        if ahap.is_none() {
            ahap = Some(Ahap::new(description.clone(), name.clone()));
        }
        let pattern = ahap.as_mut().unwrap();
        let ctx = MusicalContext::new(bpm, 4, 4);

        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            if c.is_whitespace() {
                continue;
            }

            let accented = c == '!';
            let c = if accented {
                match chars.next() {
                    Some(next) => next,
                    None => return Err("`!` at end of line with nothing to accent".into()),
                }
            } else {
                c
            };

            if c == '<' {
                octave -= 1;
                continue;
            }
            if c == '>' {
                octave += 1;
                continue;
            }

            if c == '-' {
                let denom = read_duration_digits(&mut chars, default_denominator, &mut default_denominator, duration_mode);
                time += ctx.beat_to_seconds(4.0 / denom as f64);
                continue;
            }

            match section {
                Section::Melody => {
                    let sharp = matches!(chars.peek(), Some('#')) && {
                        chars.next();
                        true
                    };
                    if note_semitone(c).is_none() {
                        return Err(format!("unknown symbol in melody body: {c:?}"));
                    }
                    let denom =
                        read_duration_digits(&mut chars, default_denominator, &mut default_denominator, duration_mode);
                    let duration = ctx.beat_to_seconds(4.0 / denom as f64);

                    let mut freq = note_freq(c, sharp, octave);
                    if !(80.0..=230.0).contains(&freq) {
                        eprintln!(
                            "warning: note {c}{} at octave {octave} ({freq:.1} Hz) is outside the 80-230 Hz \
                             haptic range, clamping",
                            if sharp { "#" } else { "" }
                        );
                        freq = freq.clamp(80.0, 230.0);
                    }
                    let sharpness = freq_to_sharpness(freq, true)?;
                    let mut intensity = dynamics / 10.0;
                    if accented {
                        intensity += accent_delta;
                    }
                    intensity = intensity.clamp(0.0, 1.0);

                    pattern.add_event(Continuous::at(time, duration).intensity(intensity).sharpness(sharpness).build());
                    time += duration;
                }
                Section::Drums => {
                    let denom =
                        read_duration_digits(&mut chars, default_denominator, &mut default_denominator, duration_mode);
                    let duration = ctx.beat_to_seconds(4.0 / denom as f64);
                    let shape = drum_shape(c).ok_or_else(|| format!("unknown symbol in drum body: {c:?}"))?;

                    let mut intensity = dynamics / 10.0;
                    if accented {
                        intensity += accent_delta;
                    }
                    intensity = intensity.clamp(0.0, 1.0);

                    match shape {
                        DrumShape::Punch { sharpness } => {
                            let event = Continuous::at(time, duration)
                                .intensity(intensity)
                                .sharpness(sharpness)
                                .attack(0.0)
                                .decay(duration * 0.6)
                                .release(duration * 0.4)
                                .build();
                            pattern.add_event(event);
                        }
                        DrumShape::Hit { sharpness } => {
                            pattern.add_event(Transient::at(time).intensity(intensity).sharpness(sharpness).build());
                        }
                        DrumShape::Ring { sharpness } => {
                            pattern.add_event(
                                Continuous::at(time, duration).intensity(intensity).sharpness(sharpness).build(),
                            );
                            let curve = Curve::new(CURVE_HAPTIC_INTENSITY, time)
                                .anchor(0.0, 1.0)
                                .ease_in_out_to((0.0, 1.0), (duration, 0.0), 6)
                                .build();
                            pattern.add_parameter_curve(curve);
                        }
                    }
                    time += duration;
                }
                Section::None => {
                    return Err("note/drum symbol before @melody or @drums section".into());
                }
            }
        }
    }

    ahap.ok_or_else(|| "empty pattern: no @melody or @drums body found".to_string())
}

/// Consumes leading digits (if any) from `chars` as a duration override,
/// returning the resolved denominator to use for *this* symbol, and - in
/// `Sticky` mode - updating `running_default` for every symbol after it.
fn read_duration_digits(
    chars: &mut std::iter::Peekable<std::str::Chars>,
    running_default: u32,
    running_default_slot: &mut u32,
    mode: DurationMode,
) -> u32 {
    let mut digits = String::new();
    while let Some(d) = chars.peek() {
        if d.is_ascii_digit() {
            digits.push(*d);
            chars.next();
        } else {
            break;
        }
    }
    if digits.is_empty() {
        return running_default;
    }
    let denom: u32 = digits.parse().unwrap_or(running_default);
    if mode == DurationMode::Sticky {
        *running_default_slot = denom;
    }
    denom
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_frequencies_match_scientific_pitch() {
        // A4 = 440 Hz exactly.
        assert!((note_freq('A', false, 4) - 440.0).abs() < 1e-6);
        // C4 (middle C) ~ 261.63 Hz.
        assert!((note_freq('C', false, 4) - 261.6256).abs() < 1e-3);
    }

    #[test]
    fn simple_melody_parses_without_error() {
        let src = "@tempo 200\n@octave 3\n@melody\n@f\nEE-E-CE-\n";
        let ahap = parse_melody(src).unwrap();
        assert!(!ahap.pattern.is_empty());
    }

    #[test]
    fn octave_shift_clamps_out_of_range_notes() {
        // G4 is ~392 Hz, well above the 230 Hz ceiling - must clamp, not error.
        let src = "@tempo 120\n@octave 3\n@melody\n>G\n";
        let ahap = parse_melody(src).unwrap();
        assert_eq!(ahap.pattern.len(), 1);
    }

    #[test]
    fn duration_digits_are_temporary_by_default() {
        // F#4 uses quarter (denom 4) just for that note; the next note goes
        // back to the eighth-note default.
        let src = "@tempo 120\n@octave 3\n@melody\nC F#4 C\n";
        let ahap = parse_melody(src).unwrap();
        assert_eq!(ahap.pattern.len(), 3);
    }

    #[test]
    fn drum_body_accepts_all_letters() {
        let src = "@tempo 120\n@drums\nk s h x o c r t\n";
        let ahap = parse_melody(src).unwrap();
        // k,t (punch) + s,h,x (hit) = 5 plain events; o,c,r (ring) each add
        // an event *and* a parameter curve = 6 more pattern items. 5+6=11.
        assert_eq!(ahap.pattern.len(), 11);
        let event_count = ahap.pattern.iter().filter(|p| p.event.is_some()).count();
        assert_eq!(event_count, 8);
    }

    #[test]
    fn unknown_directive_is_an_error() {
        let src = "@bogus 1\n@melody\nC\n";
        assert!(parse_melody(src).is_err());
    }
}
