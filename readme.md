# ahap_rs

A Rust library + CLI toolset for building Apple Haptic and Audio Pattern (`.ahap`) files.

This started as a single bike-engine-sound demo. It's grown into a Rust
toolset covering the same ground as the Go
[`apple_haptic_creator`](https://github.com/denizsincar29/apple_haptic_creator)
project (MIDI conversion, a REPL, hand-built demos) plus its own text DSL for
writing patterns directly (`.msh`, replacing the older separate `haptrack`
format - see `msh2ahap` below). All CLIs use `clap` derive for argument
parsing, so `--help`/`--version` work everywhere.

## Binaries

- **`midi2ahap`** - converts a `.mid` file to `.ahap`. Channel-10 (GM drum
  channel) notes get instrument-appropriate haptic shapes instead of a single
  flat transient: kicks/toms are a short felt punch, cymbals/open hi-hat get
  a long ringing tail with a decaying intensity curve, snares/sticks stay a
  crisp instantaneous hit. Melodic notes map pitch to sharpness, and notes
  below the Taptic Engine's ~80Hz floor are split into two simultaneous
  notes (root + a fourth below) since a single out-of-range tone doesn't
  read as a pitch.

  `--no-drums` drops channel 10 entirely; `--drums-as-melody` treats it as
  regular melodic notes instead; `--debug-channels` prints a note-on count
  per channel so you can check what's actually in a file.

  Attack/decay/release and sharpness can also be steered *from inside the
  MIDI file itself* using the standard General MIDI 2 Sound Controller CCs:
  CC 73 (Attack Time), CC 72 (Release Time), CC 75 (Decay Time), CC 74
  (Brightness -> sharpness offset). These are real GM2 CCs, not invented
  ones, so any DAW can already draw automation for them - draw a CC73 ramp
  and every event converted after that point gets the new attack time. The
  values are global (apply to every subsequent event on every channel/track,
  not just the one the CC was sent on) and are *relative*: a CC maps to a
  fraction (0.0-1.0) of each event's own duration, not an absolute number of
  seconds, so a large release never smears a short note into a longer hum
  than the note itself. Brightness (CC74) maps 0-127 -> +/-0.3 sharpness
  offset linearly.

  ```bash
  cargo run --release --bin midi2ahap -- song.mid song.ahap
  ```

- **`msh2ahap`** - compiles a `.msh` (Music Haptics) file into `.ahap`. Handy
  for writing a pattern by ear instead of exporting MIDI from a DAW, or for
  hand-authoring UI/motor feedback as a readable event list instead of raw
  timestamps. Full format documented in [`ahap_rs::msh`](src/msh.rs); the
  short version:

  ```
  @tempo 200
  @octave 3
  @melody
  @f
  EE-E-CE- !G---<G---
  (DE) !Bb4
  @drums
  k-s-k-s- !k-s-k-!s-
  @events
  repeat transient t=0.45 count=7 step=0.05 intensity=1.0 sharpness=0.3
  ```

  `@melody`: `A`-`G` are notes (`#` for sharp, `b` for flat - `Bb`, `Eb`,
  etc, real chord-chart spelling), `-` is a rest, `!` before a note/rest/tied
  group accents it, `<`/`>` shift the octave down/up, and digits after a
  note override its duration for that symbol (or become the new default in
  `@duration-mode sticky`). Frequencies are clamped to the Taptic Engine's
  80-230 Hz range, with a warning on stderr if a note would have gone
  outside it. Notes inside `(...)` are tied into one continuous event that
  pitch-bends between them - `(DE)` holds D, then glides into E over the
  last `@curve-transition` fraction (default 10%) of D's own duration.

  `@drums` switches to a small letter-based kit (`k`ick, `t`om, `s`nare,
  `h`i-hat, e`x`(clap), `o`pen hi-hat, `c`rash, `r`ide) using the same
  rest/accent/duration syntax as melody.

  `@events` is for non-melodic haptics (motor rumbles, UI feedback) as a
  line-based DSL instead of notes: `transient`/`continuous`/`repeat`/`curve`,
  each a kind followed by `key=value` pairs.

  ```bash
  cargo run --release --bin msh2ahap -- song.msh song.ahap
  ```

- **`ahap_repl`** (formerly `ahapgen`) - interactive REPL for building a
  pattern by hand (`t`/`c`/`beat`/`bar`/`export` commands).

  ```bash
  cargo run --release --bin ahap_repl -- -o mine.ahap
  ```

- **`bike_demo`** (formerly `makeahap`) - the original motorcycle-sound demo
  pattern, kept as a worked example of hand-building a pattern with `Builder`.

  ```bash
  cargo run --release --bin bike_demo -- --output bike.ahap
  ```

Run any binary with `--help` for its full option list.

Since iOS 17, `.ahap` files can be previewed directly via Quick Look, so they
open straight from the Files app or from messaging apps that support file
previews (Telegram, WhatsApp, etc).

## Examples

`examples/` has:

- **`doom.mid` / `doom.ahap`** - the Doom soundtrack, converted with `midi2ahap`.
- **`mario.msh` / `mario.ahap`** - a short hand-written `.msh` pattern
  (melody + a tied pitch-bend group + drums + an `@events` section) showing
  off the format, converted with `msh2ahap`.

Regenerate either with:

```bash
cargo run --release --bin midi2ahap -- examples/doom.mid examples/doom.ahap
cargo run --release --bin msh2ahap -- examples/mario.msh examples/mario.ahap
```

## Library

`ahap_rs` also works as a plain library if you want to build patterns from
your own Rust code:

```rust
use ahap_rs::{Ahap, Transient, Continuous};

let mut ahap = Ahap::new("my pattern", "me");
ahap.add_event(Transient::at(0.0).intensity(1.0).sharpness(0.5).build());
ahap.add_event(Continuous::at(0.5, 0.2).intensity(0.8).sharpness(0.6).build());

// Build a family of events from one template instead of writing a loop
// with a running time variable:
let buzz = Transient::at(0.0).intensity(0.8).sharpness(0.6).build();
ahap.add_repeated(&buzz, 7, 0.05); // 7 transients, 50ms apart

// Events are immutable - `with_*` returns a modified copy, the original
// is untouched:
let louder = buzz.with_intensity(1.0).with_time(1.0);

ahap.export("out.ahap", true).unwrap();
```

Every public item has a doc comment - run `cargo doc --open` for the full
generated reference.

## Notable differences from the Go version

- `midi2ahap`'s tempo handling integrates elapsed time per tempo segment
  instead of recomputing all-elapsed-ticks at whatever tempo is current,
  which mattered for MIDI files with tempo changes mid-track.
- The Go version's separate `haptrack` DSL (`.hap` files) has been replaced
  by `.msh` (`msh2ahap`), which covers the same drum-pattern ground plus
  melody, tied/pitch-bend groups, and a line-based `@events` DSL for
  non-melodic haptics, all in one format.
- Envelope (Attack/Decay/Release) and curve-anchor helpers are built in from
  the start, used by `midi2ahap`'s drum rendering.

## Tests

```bash
cargo test
```

## License

MIT.

## Contributing

Contributions are welcome! Please feel free to submit a pull request or open
an issue.
