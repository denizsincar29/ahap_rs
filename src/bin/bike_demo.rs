//! # bike_demo
//!
//! Generates the original motorcycle-engine-sound demo AHAP pattern - the
//! first thing this crate ever did, kept around as a worked example of
//! hand-building a pattern with [`Builder`]. It's not a general-purpose
//! tool; it just writes one fixed hardcoded pattern (initial rumble, gear
//! shift, then 15s of "engine running" transients shaped by a handful of
//! sharpness curves for acceleration) to the output file.
//!
//! Usage: `bike_demo [--output bike.ahap] [--indent]`

use ahap_rs::{Builder, CURVE_HAPTIC_SHARPNESS};
use clap::Parser;

/// Generates the original motorcycle-engine-sound demo AHAP pattern.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Output .ahap file
    #[arg(long, default_value = "bike.ahap")]
    output: String,

    /// Pretty-print the output JSON
    #[arg(long)]
    indent: bool,
}

fn main() {
    let cli = Cli::parse();
    let output = cli.output;
    let indent = cli.indent;

    println!("Creating motorcycle sound haptic pattern...");

    let mut builder = Builder::new("bike sound", "Deniz Sincar");

    // Initial rumble
    let mut time = 0.0;
    let dur = 0.4;
    builder.add_continuous(time, dur, 0.5, 0.4);
    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time, vec![(0.0, 0.4), (0.4, 0.75)], 10);

    // Gear shift: quick transients
    time = 0.45;
    for _ in 0..7 {
        builder.add_transient(time, 1.0, 0.3);
        time += 0.05;
    }

    // Main engine running (15s continuous + rapid transients)
    builder.add_continuous(time, 15.0, 0.75, 0.0);
    for i in 0..300 {
        builder.add_transient(time + i as f64 * 0.05, 1.0, 1.0);
    }

    // Sharpness curves for engine acceleration
    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time, vec![(0.0, 0.0), (0.4, 0.75)], 10);
    time += 0.4;

    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time, vec![(0.0, 0.75), (0.8, 0.2)], 10);
    time += 0.8;

    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time, vec![(0.0, 0.0), (3.0, 0.5)], 10);
    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time + 3.0, vec![(0.0, 0.2), (3.0, 0.65)], 10);
    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time + 6.0, vec![(0.0, 0.4), (4.0, 1.0)], 10);
    builder.add_curve(CURVE_HAPTIC_SHARPNESS, time + 10.0, vec![(0.0, 1.0), (2.0, 0.0)], 10);

    if let Err(e) = builder.build().export(&output, indent) {
        eprintln!("Failed to export AHAP: {e}");
        std::process::exit(1);
    }
    println!("Successfully created {output}");
}
