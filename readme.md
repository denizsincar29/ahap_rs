# ahap_rs

A Rust library + CLI toolset for building Apple Haptic and Audio Pattern (`.ahap`) files.

This started as a single bike-engine-sound demo. It's now a proper Rust port
of the Go [`apple_haptic_creator`](https://github.com/denizsincar29/apple_haptic_creator)
project: same four tools, same AHAP output shape, plus a couple of bugs the
Go version had that are fixed here (see below). All four CLIs use `clap`
derive for argument parsing, so `--help`/`--version` work everywhere.

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
  not just the one the CC was sent on), and each value is mapped 0-127 ->
  0.0-1.0 seconds (72/73/75) or +/-0.3 sharpness offset (74) linearly.

  ```bash
  cargo run --release --bin midi2ahap -- song.mid song.ahap
  ```

- **`haptrack`** - compiles the haptrack DSL (`.hap` text files defining
  drum-style patterns with letters, note durations, and curves) to `.ahap`.

  ```bash
  cargo run --release --bin haptrack -- pattern.hap pattern.ahap
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

## Library

`ahap_rs` also works as a plain library if you want to build patterns from
your own Rust code:

```rust
use ahap_rs::{Ahap, Transient, Continuous};

let mut ahap = Ahap::new("my pattern", "me");
ahap.add_event(Transient::at(0.0).intensity(1.0).sharpness(0.5).build());
ahap.add_event(Continuous::at(0.5, 0.2).intensity(0.8).sharpness(0.6).build());
ahap.export("out.ahap", true).unwrap();
```

Every public item has a doc comment - run `cargo doc --open` for the full
generated reference.

## Notable differences from the Go version

- `midi2ahap`'s tempo handling integrates elapsed time per tempo segment
  instead of recomputing all-elapsed-ticks at whatever tempo is current,
  which mattered for MIDI files with tempo changes mid-track.
- `haptrack`'s definition parser accepts both the DSL's newer
  `name: type; key=value` syntax and the older comma-separated syntax
  (`name, intensity, sharpness[, curve_direction, duration_ms]`) that the
  shipped example `.hap` files actually use - the Go parser only understood
  the new syntax, so real example files silently fell back to default
  intensity/sharpness values.
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
