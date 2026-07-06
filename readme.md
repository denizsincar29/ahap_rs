# ahap_rs

A Rust library + CLI toolset for building Apple Haptic and Audio Pattern (`.ahap`) files.

This started as a single bike-engine-sound demo. It's now a proper Rust port
of the Go [`apple_haptic_creator`](https://github.com/denizsincar29/apple_haptic_creator)
project: same four tools, same AHAP output shape, plus a couple of bugs the
Go version had that are fixed here (see below).

## Binaries

- **`midi2ahap`** - converts a `.mid` file to `.ahap`. Channel-10 (GM drum
  channel) notes get instrument-appropriate haptic shapes instead of a single
  flat transient: kicks/toms are a short felt punch, cymbals/open hi-hat get
  a long ringing tail with a decaying intensity curve, snares/sticks stay a
  crisp instantaneous hit. Melodic notes map pitch to sharpness.

  ```bash
  cargo run --release --bin midi2ahap -- song.mid song.ahap
  ```

- **`haptrack`** - compiles the haptrack DSL (`.hap` text files defining
  drum-style patterns with letters, note durations, and curves) to `.ahap`.

  ```bash
  cargo run --release --bin haptrack -- pattern.hap pattern.ahap
  ```

- **`ahapgen`** - interactive REPL for building a pattern by hand
  (`t`/`c`/`beat`/`bar`/`export` commands).

  ```bash
  cargo run --release --bin ahapgen -- -o mine.ahap
  ```

- **`makeahap`** - the original bike-engine-sound demo.

  ```bash
  cargo run --release --bin makeahap -- -output bike.ahap
  ```

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
