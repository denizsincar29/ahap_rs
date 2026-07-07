//! # melody2ahap
//!
//! Compiles a `.hmel` haptic melody file (note letters + rests, no time
//! signature - see [`ahap_rs::melody`] for the full format) into an `.ahap`
//! file.

use ahap_rs::melody::parse_melody;
use clap::Parser;
use std::{fs, process};

#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Input .hmel file
    input: String,
    /// Output .ahap file
    output: String,
}

fn main() {
    let cli = Cli::parse();

    let source = match fs::read_to_string(&cli.input) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {e}", cli.input);
            process::exit(1);
        }
    };

    let ahap = match parse_melody(&source) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Failed to parse {}: {e}", cli.input);
            process::exit(1);
        }
    };

    let event_count = ahap.pattern.len();

    if let Err(e) = ahap.export(&cli.output, false) {
        eprintln!("Failed to export AHAP: {e}");
        process::exit(1);
    }

    println!("Successfully created {}", cli.output);
    println!("Total haptic events: {event_count}");
}
