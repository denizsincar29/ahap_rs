//! # Music Haptics format (`.msh`)
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
//! @octave 3        // default octave
//! @accent-velocity +0.2   /* can be with or without +;
//!                            a negative value reverses accents */
//! @melody
//! @f               // forte: dynamics level 8.0/10
//! EE-E-CE- !G---<G---
//! @drums
//! k-s-k-s- !k-s-k-!s-
//! ```
//!
//! ## Directives
//! All start with `@`, optionally followed by whitespace before the keyword
//! (`@tempo` and `@ tempo` both work). `//` starts a line comment running to
//! end of line; `/* ... */` is a block comment that can span multiple
//! lines. `#` is never a comment marker here - it's reserved for sharp
//! notes (`F#`), same as written music.
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
//! - `@curve-transition <fraction>` - how much of a tied note's own
//!   duration (see "Tied/pitch-bend groups" below) is spent gliding into
//!   the next note, `0.0`-`1.0`. Default `0.1`.
//! - Dynamics markings, each sets the current intensity level on a 0-10
//!   scale (normalized to 0.0-1.0 for AHAP by dividing by 10):
//!   `@pp` = 2.0, `@p` = 4.0, `@mp` = 5.0, `@mf` = 6.0, `@f`/`@forte` = 8.0,
//!   `@ff` = 10.0. Default is `@mf` (6.0).
//! - `@melody` / `@drums` / `@events` - switch the following body text into
//!   melody, drum, or event mode. Can appear more than once to interleave
//!   sections; state like octave/tempo/dynamics carries over between them.
//!
//! ## Melody body
//! - `A`-`G` (uppercase only) are notes. A `#` right after the letter makes
//!   it sharp (`F#`); a `b` right after makes it flat (`Bb`) - lowercase is
//!   never a note name on its own, only ever this flat modifier, so there's
//!   no ambiguity with the note B.
//! - `-` is a rest.
//! - `!` right before a note, rest, or tied group is an accent (see
//!   `@accent-velocity`).
//! - `<` shifts the current octave down one, `>` shifts it up one. Shifts
//!   persist until the next shift or `@octave` directive.
//! - Digits right after a note/rest override its duration for just that
//!   symbol (or the running default, if `@duration-mode sticky`); see
//!   `@duration` for the accepted values.
//! - A computed frequency is clamped to the Taptic Engine's 80-230 Hz
//!   sharpness range (a warning is printed to stderr when that happens) -
//!   notes are never allowed to render outside of it.
//!
//! ### Tied/pitch-bend groups
//! Notes inside `(...)` are tied into a *single* continuous event spanning
//! their combined duration, with a sharpness curve that glides between
//! them: `(DE)` plays a D-eighth tied to an E-eighth (0.25s + 0.25s = one
//! quarter note total by default) - D holds for the first 90% of its own
//! duration, then bends into E over the last 10% (`@curve-transition`
//! controls that fraction). Longer groups chain the same glide between each
//! consecutive pair: `(BDA)` holds B, bends into D, holds D, bends into A,
//! holds A to the end. Each note inside the parens accepts its own sharp/flat
//! and duration digits exactly like a standalone note.
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
//!
//! ## Events body
//! For non-melodic haptics (motor rumbles, UI feedback - anything better
//! described as "this happens at time T" than as notes on a staff), one
//! declarative line per event: a kind, then `key=value` pairs.
//!
//! ```text
//! @events
//! transient t=0.0 intensity=0.8 sharpness=0.5
//! continuous t=0.1 duration=0.2 intensity=0.6 sharpness=0.4 attack=0.01 release=0.05
//! repeat transient t=0.45 count=7 step=0.05 intensity=1.0 sharpness=0.3
//! curve sharpness t=0.0 duration=0.4 from=0.4 to=0.75
//! ```
//!
//! - `transient t=<seconds> [intensity=<0-1>] [sharpness=<0-1>]`
//! - `continuous t=<seconds> duration=<seconds> [intensity=<0-1>]
//!   [sharpness=<0-1>] [attack=<seconds>] [decay=<seconds>] [release=<seconds>]`
//! - `repeat transient|continuous t=<seconds> count=<n> step=<seconds>
//!   [intensity=<0-1>] [sharpness=<0-1>] [duration=<seconds>]` - see
//!   [`crate::Ahap::add_repeated`] for the underlying mechanism.
//! - `curve intensity|sharpness t=<seconds> duration=<seconds> from=<0-1>
//!   to=<0-1> [steps=<n>]`
//!
//! `intensity`/`sharpness` default to `1.0`/`0.5` when omitted.

use crate::{
    freq_to_sharpness, Ahap, Continuous, Curve, MusicalContext, Transient, CURVE_HAPTIC_INTENSITY,
    CURVE_HAPTIC_SHARPNESS,
};
use std::collections::HashMap;

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

/// Semitone offset from C for a note letter. Note letters are uppercase
/// only (`A`-`G`) - lowercase is reserved for the flat modifier (`Eb`),
/// so there's no ambiguity between "the note B" and "a flat".
fn note_semitone(letter: char) -> Option<i32> {
    match letter {
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

/// Frequency in Hz for a note letter (+ optional accidental) at a given
/// octave, scientific pitch notation (`C4` = MIDI 60 = middle C, `A4` =
/// 440 Hz). `accidental` is `1` for sharp, `-1` for flat, `0` for natural.
fn note_freq(letter: char, accidental: i32, octave: i32) -> f64 {
    let semitone = note_semitone(letter).unwrap_or(0) + accidental;
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
    Events,
}

/// Removes `/* ... */` block comments (which may span multiple lines)
/// before line-by-line parsing. Everything inside is replaced with spaces
/// (newlines kept as newlines) so line numbers in any future error
/// reporting stay accurate.
fn strip_block_comments(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let bytes = source.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
            let mut j = i + 2;
            while j < bytes.len() && !(bytes[j] == b'*' && bytes.get(j + 1) == Some(&b'/')) {
                if bytes[j] == b'\n' {
                    out.push('\n');
                } else {
                    out.push(' ');
                }
                j += 1;
            }
            i = (j + 2).min(bytes.len());
        } else {
            // Advance by whole UTF-8 char boundaries, not raw bytes.
            let ch = source[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Parses `.msh` (Music Haptics) source text into an [`Ahap`] pattern.
pub fn parse_msh(source: &str) -> Result<Ahap, String> {
    let source = strip_block_comments(source);
    let mut name = String::from("untitled");
    let mut description = String::from("music haptics pattern");
    let mut bpm = 120.0;
    let mut octave = 4;
    let mut default_denominator = 8; // eighth, per the format's built-in default
    let mut duration_mode = DurationMode::Temporary;
    let mut accent_delta = 0.15;
    let mut dynamics = 6.0; // mf
    let mut curve_transition = 0.1; // fraction of a tied note's own duration spent gliding to the next
    let mut section = Section::None;
    let mut time = 0.0;
    let mut ahap: Option<Ahap> = None;

    for raw_line in source.lines() {
        // `//` starts a line comment; `#` is never a comment here, it's
        // reserved for sharp notes (`F#`). Block comments were already
        // stripped above.
        let cut = raw_line.find("//").unwrap_or(raw_line.len());
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
                "curve-transition" | "curve_transition" => {
                    curve_transition = args
                        .parse::<f64>()
                        .map_err(|_| format!("bad @curve_transition value: {args:?}"))?
                        .clamp(0.0, 1.0);
                }
                "melody" => section = Section::Melody,
                "drums" => section = Section::Drums,
                "events" => section = Section::Events,
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

        if section == Section::Events {
            parse_event_line(line, pattern)?;
            continue;
        }

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

            if c == '(' {
                if section != Section::Melody {
                    return Err("`(...)` tied/pitch-bend groups are only valid inside @melody".into());
                }
                let mut notes: Vec<(f64, f64)> = Vec::new(); // (sharpness, duration seconds)
                loop {
                    match chars.next() {
                        None => return Err("unterminated `(` tied group".into()),
                        Some(')') => break,
                        Some(nc) if nc.is_whitespace() => continue,
                        Some(nc) => {
                            if note_semitone(nc).is_none() {
                                return Err(format!("unknown symbol inside tied group: {nc:?}"));
                            }
                            let accidental = if matches!(chars.peek(), Some('#')) {
                                chars.next();
                                1
                            } else if matches!(chars.peek(), Some('b')) {
                                chars.next();
                                -1
                            } else {
                                0
                            };
                            let denom = read_duration_digits(
                                &mut chars,
                                default_denominator,
                                &mut default_denominator,
                                duration_mode,
                            );
                            let duration = ctx.beat_to_seconds(4.0 / denom as f64);
                            let mut freq = note_freq(nc, accidental, octave);
                            if !(80.0..=230.0).contains(&freq) {
                                eprintln!(
                                    "warning: tied note {nc}{} at octave {octave} ({freq:.1} Hz) is outside the \
                                     80-230 Hz haptic range, clamping",
                                    match accidental {
                                        1 => "#",
                                        -1 => "b",
                                        _ => "",
                                    }
                                );
                                freq = freq.clamp(80.0, 230.0);
                            }
                            notes.push((freq_to_sharpness(freq, true)?, duration));
                        }
                    }
                }
                if notes.is_empty() {
                    return Err("empty `()` tied group".into());
                }

                let total_duration: f64 = notes.iter().map(|(_, d)| *d).sum();
                let mut intensity = dynamics / 10.0;
                if accented {
                    intensity += accent_delta;
                }
                intensity = intensity.clamp(0.0, 1.0);

                pattern.add_event(
                    Continuous::at(time, total_duration).intensity(intensity).sharpness(notes[0].0).build(),
                );

                // Hold each note's sharpness flat, then glide into the next
                // one during the last `curve_transition` fraction of its own
                // duration - a D-eighth tied to an E-eighth (`(DE)`) plays D
                // for 90% of its duration and pitch-bends into E for the
                // last 10%, by default.
                let mut curve = Curve::new(CURVE_HAPTIC_SHARPNESS, time);
                let mut cursor = 0.0;
                for (i, (sharpness, duration)) in notes.iter().enumerate() {
                    let hold_end = cursor + duration * (1.0 - curve_transition);
                    curve = curve.anchor(cursor, *sharpness).anchor(hold_end, *sharpness);
                    if let Some((next_sharpness, _)) = notes.get(i + 1) {
                        curve = curve.ease_in_out_to((hold_end, *sharpness), (cursor + duration, *next_sharpness), 6);
                    }
                    cursor += duration;
                }
                pattern.add_parameter_curve(curve.build());

                time += total_duration;
                continue;
            }

            match section {
                Section::Melody => {
                    let accidental = if matches!(chars.peek(), Some('#')) {
                        chars.next();
                        1
                    } else if matches!(chars.peek(), Some('b')) {
                        chars.next();
                        -1
                    } else {
                        0
                    };
                    if note_semitone(c).is_none() {
                        return Err(format!("unknown symbol in melody body: {c:?}"));
                    }
                    let denom =
                        read_duration_digits(&mut chars, default_denominator, &mut default_denominator, duration_mode);
                    let duration = ctx.beat_to_seconds(4.0 / denom as f64);

                    let mut freq = note_freq(c, accidental, octave);
                    if !(80.0..=230.0).contains(&freq) {
                        eprintln!(
                            "warning: note {c}{} at octave {octave} ({freq:.1} Hz) is outside the 80-230 Hz \
                             haptic range, clamping",
                            match accidental {
                                1 => "#",
                                -1 => "b",
                                _ => "",
                            }
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
                    return Err("note/drum symbol before @melody, @drums, or @events section".into());
                }
                Section::Events => unreachable!("handled by the early `continue` above"),
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

/// Parses one `@events` body line into key=value pairs. The first
/// whitespace-separated token is the event kind and is not part of the map.
fn parse_kv<'a>(tokens: impl Iterator<Item = &'a str>) -> Result<HashMap<&'a str, &'a str>, String> {
    let mut map = HashMap::new();
    for tok in tokens {
        let (k, v) = tok.split_once('=').ok_or_else(|| format!("expected key=value, got {tok:?}"))?;
        map.insert(k, v);
    }
    Ok(map)
}

fn kv_f64(map: &HashMap<&str, &str>, key: &str, default: Option<f64>) -> Result<f64, String> {
    match map.get(key) {
        Some(v) => v.parse().map_err(|_| format!("bad {key} value: {v:?}")),
        None => default.ok_or_else(|| format!("missing required key: {key}")),
    }
}

/// Parses and executes one line of the `@events` section - a line-based DSL
/// for non-melodic haptics (motor rumbles, UI feedback, anything better
/// described as "an event happens at time T" than as notes on a staff).
/// Four kinds, each a first token followed by `key=value` pairs:
///
/// - `transient t=<seconds> [intensity=<0-1>] [sharpness=<0-1>]`
/// - `continuous t=<seconds> duration=<seconds> [intensity=<0-1>]
///   [sharpness=<0-1>] [attack=<seconds>] [decay=<seconds>] [release=<seconds>]`
/// - `repeat transient|continuous t=<seconds> count=<n> step=<seconds>
///   [intensity=<0-1>] [sharpness=<0-1>] [duration=<seconds>]` - see
///   [`Ahap::add_repeated`] for the mechanism this compiles down to.
/// - `curve intensity|sharpness t=<seconds> duration=<seconds> from=<0-1>
///   to=<0-1> [steps=<n>]`
///
/// `intensity`/`sharpness` default to `1.0`/`0.5` when omitted.
fn parse_event_line(line: &str, pattern: &mut Ahap) -> Result<(), String> {
    let mut tokens = line.split_whitespace();
    let kind = tokens.next().ok_or("empty @events line")?;

    match kind {
        "transient" => {
            let map = parse_kv(tokens)?;
            let t = kv_f64(&map, "t", None)?;
            let intensity = kv_f64(&map, "intensity", Some(1.0))?;
            let sharpness = kv_f64(&map, "sharpness", Some(0.5))?;
            pattern.add_event(Transient::at(t).intensity(intensity).sharpness(sharpness).build());
        }
        "continuous" => {
            let map = parse_kv(tokens)?;
            let t = kv_f64(&map, "t", None)?;
            let duration = kv_f64(&map, "duration", None)?;
            let intensity = kv_f64(&map, "intensity", Some(1.0))?;
            let sharpness = kv_f64(&map, "sharpness", Some(0.5))?;
            let mut builder = Continuous::at(t, duration).intensity(intensity).sharpness(sharpness);
            if let Some(v) = map.get("attack") {
                builder = builder.attack(v.parse().map_err(|_| format!("bad attack value: {v:?}"))?);
            }
            if let Some(v) = map.get("decay") {
                builder = builder.decay(v.parse().map_err(|_| format!("bad decay value: {v:?}"))?);
            }
            if let Some(v) = map.get("release") {
                builder = builder.release(v.parse().map_err(|_| format!("bad release value: {v:?}"))?);
            }
            pattern.add_event(builder.build());
        }
        "repeat" => {
            let sub_kind = tokens.next().ok_or("`repeat` needs a sub-event kind: transient or continuous")?;
            let map = parse_kv(tokens)?;
            let t = kv_f64(&map, "t", None)?;
            let count = kv_f64(&map, "count", None)? as usize;
            let step = kv_f64(&map, "step", None)?;
            let intensity = kv_f64(&map, "intensity", Some(1.0))?;
            let sharpness = kv_f64(&map, "sharpness", Some(0.5))?;
            let base = match sub_kind {
                "transient" => Transient::at(t).intensity(intensity).sharpness(sharpness).build(),
                "continuous" => {
                    let duration = kv_f64(&map, "duration", Some(0.1))?;
                    Continuous::at(t, duration).intensity(intensity).sharpness(sharpness).build()
                }
                other => return Err(format!("unknown `repeat` sub-kind: {other:?}")),
            };
            pattern.add_repeated(&base, count, step);
        }
        "curve" => {
            let parameter = tokens.next().ok_or("`curve` needs a parameter: intensity or sharpness")?;
            let param_id = match parameter {
                "intensity" => CURVE_HAPTIC_INTENSITY,
                "sharpness" => CURVE_HAPTIC_SHARPNESS,
                other => return Err(format!("unknown curve parameter: {other:?}")),
            };
            let map = parse_kv(tokens)?;
            let t = kv_f64(&map, "t", None)?;
            let duration = kv_f64(&map, "duration", None)?;
            let from = kv_f64(&map, "from", None)?;
            let to = kv_f64(&map, "to", None)?;
            let steps = match map.get("steps") {
                Some(v) => v.parse::<usize>().map_err(|_| format!("bad steps value: {v:?}"))?,
                None => 10,
            };
            let curve = Curve::new(param_id, t).ease_in_out_to((0.0, from), (duration, to), steps).build();
            pattern.add_parameter_curve(curve);
        }
        other => return Err(format!("unknown @events kind: {other:?}")),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn note_frequencies_match_scientific_pitch() {
        // A4 = 440 Hz exactly.
        assert!((note_freq('A', 0, 4) - 440.0).abs() < 1e-6);
        // C4 (middle C) ~ 261.63 Hz.
        assert!((note_freq('C', 0, 4) - 261.6256).abs() < 1e-3);
    }

    #[test]
    fn flat_lowers_by_one_semitone() {
        // Bb4 must equal the frequency of A#4 (same pitch, different spelling).
        let bb4 = note_freq('B', -1, 4);
        let a_sharp_4 = note_freq('A', 1, 4);
        assert!((bb4 - a_sharp_4).abs() < 1e-9);
    }

    #[test]
    fn flat_note_parses_in_melody_body() {
        let src = "@tempo 120\n@octave 4\n@melody\nBb\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 1);
    }

    #[test]
    fn tied_group_produces_one_event_and_one_curve() {
        let src = "@tempo 120\n@octave 3\n@melody\n(DE)\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 2); // one Continuous event + one ParameterCurve
        let event_count = ahap.pattern.iter().filter(|p| p.event.is_some()).count();
        assert_eq!(event_count, 1);
        let event = ahap.pattern.iter().find_map(|p| p.event.as_ref()).unwrap();
        // Combined duration of two eighths = one quarter note at 120 BPM = 0.5s.
        assert!((event.event_duration.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn events_section_parses_all_four_kinds() {
        let src = "@events\n\
                   transient t=0.0 intensity=0.8 sharpness=0.5\n\
                   continuous t=0.1 duration=0.2 intensity=0.6 sharpness=0.4 attack=0.01 release=0.05\n\
                   repeat transient t=0.45 count=7 step=0.05 intensity=1.0 sharpness=0.3\n\
                   curve sharpness t=0.0 duration=0.4 from=0.4 to=0.75\n";
        let ahap = parse_msh(src).unwrap();
        // 1 transient + 1 continuous + 7 repeated transients = 9 events,
        // plus 1 parameter curve = 10 pattern items.
        assert_eq!(ahap.pattern.len(), 10);
        let event_count = ahap.pattern.iter().filter(|p| p.event.is_some()).count();
        assert_eq!(event_count, 9);
    }

    #[test]
    fn simple_melody_parses_without_error() {
        let src = "@tempo 200\n@octave 3\n@melody\n@f\nEE-E-CE-\n";
        let ahap = parse_msh(src).unwrap();
        assert!(!ahap.pattern.is_empty());
    }

    #[test]
    fn octave_shift_clamps_out_of_range_notes() {
        // G4 is ~392 Hz, well above the 230 Hz ceiling - must clamp, not error.
        let src = "@tempo 120\n@octave 3\n@melody\n>G\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 1);
    }

    #[test]
    fn duration_digits_are_temporary_by_default() {
        // F#4 uses quarter (denom 4) just for that note; the next note goes
        // back to the eighth-note default.
        let src = "@tempo 120\n@octave 3\n@melody\nC F#4 C\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 3);
    }

    #[test]
    fn drum_body_accepts_all_letters() {
        let src = "@tempo 120\n@drums\nk s h x o c r t\n";
        let ahap = parse_msh(src).unwrap();
        // k,t (punch) + s,h,x (hit) = 5 plain events; o,c,r (ring) each add
        // an event *and* a parameter curve = 6 more pattern items. 5+6=11.
        assert_eq!(ahap.pattern.len(), 11);
        let event_count = ahap.pattern.iter().filter(|p| p.event.is_some()).count();
        assert_eq!(event_count, 8);
    }

    #[test]
    fn unknown_directive_is_an_error() {
        let src = "@bogus 1\n@melody\nC\n";
        assert!(parse_msh(src).is_err());
    }

    #[test]
    fn block_comments_span_multiple_lines() {
        let src = "@tempo 120\n@octave 3\n@melody\nC /* this whole\nblock is a comment,\nspanning lines */ C\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 2);
    }

    #[test]
    fn line_comments_use_only_slash_slash() {
        let src = "@tempo 120\n@octave 3\n@melody\nC // this is a comment\nC\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 2);
    }

    #[test]
    fn hash_is_never_a_comment_marker() {
        // A bare `#` used the way a comment would be written must NOT be
        // stripped - it's reserved for sharp notes, so this is a syntax
        // error (bogus @tempo argument) rather than a silently-accepted
        // comment.
        let src = "@tempo 120 # not a comment\n@melody\nC\n";
        assert!(parse_msh(src).is_err());
    }

    #[test]
    fn sharp_note_still_works() {
        let src = "@tempo 120\n@octave 4\n@melody\nF#\n";
        let ahap = parse_msh(src).unwrap();
        assert_eq!(ahap.pattern.len(), 1);
    }
}
