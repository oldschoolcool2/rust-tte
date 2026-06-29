//! Minimal CLI wrapper around the engine's public `expand_parquet`, used only by
//! the benchmark harness (`bench/run_bench.sh`) so the Rust transform can be timed
//! and peak-RSS-measured by `/usr/bin/time -v` exactly like the R baseline.
//!
//! Usage: `runner <input.parquet> <output.parquet> [itt|pp]` (default `itt`).

use std::path::Path;
use std::process::ExitCode;

use tte_expand::{Estimand, ExpandOptions, expand_parquet};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let (Some(input), Some(output)) = (args.get(1), args.get(2)) else {
        eprintln!("usage: runner <input.parquet> <output.parquet> [itt|pp]");
        return ExitCode::FAILURE;
    };
    let estimand = match args.get(3).map(String::as_str) {
        Some("pp" | "PP") => Estimand::PerProtocol,
        _ => Estimand::Itt,
    };
    let options =
        ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand);
    match expand_parquet(Path::new(input), Path::new(output), &options) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("expand failed: {e}");
            ExitCode::FAILURE
        },
    }
}
