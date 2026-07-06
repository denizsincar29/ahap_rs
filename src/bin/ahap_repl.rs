//! # ahap_repl
//!
//! Interactive command-line REPL for building an AHAP pattern by hand, one
//! event at a time, without writing any Rust or JSON. Useful for quickly
//! sketching a pattern or poking at specific intensity/sharpness/timing
//! values to feel out how they translate to a device.
//!
//! Commands (typed at the `>` prompt):
//! - `t <time> <intensity> <sharpness>` - add a transient event
//! - `c <time> <duration> <intensity> <sharpness>` - add a continuous event
//! - `beat <beat> <intensity> <sharpness>` - transient at a beat (needs `--bpm`)
//! - `bar <bar> <intensity> <sharpness>` - transient at a bar (needs `--bpm`)
//! - `export` - write everything built so far to the output file and exit
//! - `quit` - exit without saving
//!
//! Usage: `ahap_repl [-o output.ahap] [--desc ...] [--creator ...] [--bpm N] [--time N/D]`

use ahap_rs::Builder;
use clap::Parser;
use std::io::{self, Write};

/// Interactive REPL for building an AHAP pattern by hand.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Output .ahap file
    #[arg(short, long, default_value = "output.ahap")]
    output: String,

    /// Pattern description (AHAP metadata)
    #[arg(long = "desc", default_value = "Custom haptic pattern")]
    description: String,

    /// Pattern creator (AHAP metadata)
    #[arg(long, default_value = "AHAP Generator (Rust)")]
    creator: String,

    /// BPM for musical (bar/beat) timing - enables the `beat`/`bar` commands
    #[arg(long, default_value_t = 0.0)]
    bpm: f64,

    /// Time signature as NUM/DEN, e.g. 4/4
    #[arg(long = "time", default_value = "4/4")]
    time_signature: String,
}

fn main() {
    let cli = Cli::parse();
    let output = cli.output;
    let description = cli.description;
    let creator = cli.creator;
    let bpm = cli.bpm;
    let time_signature = cli
        .time_signature
        .split_once('/')
        .map(|(n, d)| (n.parse().unwrap_or(4), d.parse().unwrap_or(4)))
        .unwrap_or((4, 4));

    let mut builder = Builder::new(description, creator);
    if bpm > 0.0 {
        builder = builder.with_bpm(bpm).with_time_signature(time_signature.0, time_signature.1);
        println!("Musical timing enabled: {:.1} BPM, {}/{} time", bpm, time_signature.0, time_signature.1);
    }

    println!("AHAP Generator (Rust) - Interactive Mode");
    println!("Commands:");
    println!("  t <time> <intensity> <sharpness>              - Add transient event");
    println!("  c <time> <duration> <intensity> <sharpness>   - Add continuous event");
    println!("  beat <beat> <intensity> <sharpness>           - Add transient at beat (requires --bpm)");
    println!("  bar <bar> <intensity> <sharpness>             - Add transient at bar (requires --bpm)");
    println!("  export                                        - Export to file and exit");
    println!("  quit                                          - Exit without saving");
    println!();

    let mut event_count = 0u32;
    let stdin = io::stdin();

    loop {
        print!("> ");
        io::stdout().flush().ok();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        match parts[0] {
            "t" | "transient" => {
                if parts.len() != 4 {
                    println!("Usage: t <time> <intensity> <sharpness>");
                    continue;
                }
                let (time, intensity, sharpness) = match parse3(&parts) {
                    Some(v) => v,
                    None => { println!("Invalid numeric argument"); continue; }
                };
                builder.add_transient(time, intensity, sharpness);
                event_count += 1;
                println!("Added transient event at {:.2}s (total: {} events)", time, event_count);
            }
            "c" | "continuous" => {
                if parts.len() != 5 {
                    println!("Usage: c <time> <duration> <intensity> <sharpness>");
                    continue;
                }
                let time: f64 = parts[1].parse().unwrap_or(0.0);
                let duration: f64 = parts[2].parse().unwrap_or(0.0);
                let intensity: f64 = parts[3].parse().unwrap_or(0.0);
                let sharpness: f64 = parts[4].parse().unwrap_or(0.0);
                builder.add_continuous(time, duration, intensity, sharpness);
                event_count += 1;
                println!("Added continuous event at {:.2}s for {:.2}s (total: {} events)", time, duration, event_count);
            }
            "beat" => {
                if bpm == 0.0 {
                    println!("Musical timing not enabled. Use -bpm flag.");
                    continue;
                }
                if parts.len() != 4 {
                    println!("Usage: beat <beat> <intensity> <sharpness>");
                    continue;
                }
                let beat: i64 = parts[1].parse().unwrap_or(0);
                let intensity: f64 = parts[2].parse().unwrap_or(0.0);
                let sharpness: f64 = parts[3].parse().unwrap_or(0.0);
                let beats_per_bar = builder.beats_per_bar() as i64;
                let bar = beat / beats_per_bar;
                let beat_in_bar = beat % beats_per_bar;
                let time = builder.at(bar, beat_in_bar);
                builder.add_transient(time, intensity, sharpness);
                event_count += 1;
                println!("Added transient at beat {beat} (bar {bar}, beat {beat_in_bar}) (total: {event_count} events)");
            }
            "bar" => {
                if bpm == 0.0 {
                    println!("Musical timing not enabled. Use -bpm flag.");
                    continue;
                }
                if parts.len() != 4 {
                    println!("Usage: bar <bar> <intensity> <sharpness>");
                    continue;
                }
                let bar: i64 = parts[1].parse().unwrap_or(0);
                let intensity: f64 = parts[2].parse().unwrap_or(0.0);
                let sharpness: f64 = parts[3].parse().unwrap_or(0.0);
                let time = builder.at(bar, 0);
                builder.add_transient(time, intensity, sharpness);
                event_count += 1;
                println!("Added transient at bar {bar} (total: {event_count} events)");
            }
            "export" | "save" => {
                if let Err(e) = builder.export(&output, true) {
                    println!("Error exporting: {e}");
                    continue;
                }
                println!("Successfully exported {event_count} events to {output}");
                return;
            }
            "quit" | "exit" | "q" => {
                println!("Exiting without saving.");
                return;
            }
            other => println!("Unknown command: {other}"),
        }
    }
}

fn parse3(parts: &[&str]) -> Option<(f64, f64, f64)> {
    Some((parts[1].parse().ok()?, parts[2].parse().ok()?, parts[3].parse().ok()?))
}
