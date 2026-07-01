// Bench-harness code: synthetic-data generation and timing. The numeric-exact /
// panic-hygiene lints that guard the engine (`src/`) are not meaningful for a
// throwaway benchmark generator, so they are relaxed here (and only here).
#![allow(
    // The criterion harness macro generates an undocumented `benches` fn.
    missing_docs,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless
)]
//! Criterion micro-benchmarks of the expansion engine.
//!
//! Measures the deterministic transform — `expand` (ITT and per-protocol) and
//! the weighted `apply_weights` path — on seeded synthetic person-time inputs
//! across row counts. Input generation happens OUTSIDE the measured closure; the
//! `LazyFrame` is cloned (cheap) per iteration. The row-count sweep is capped by
//! the `TTE_BENCH_MAX_ROWS` env var (default `1_000_000`) so CI can run a fast
//! smoke while the full sweep stays available locally:
//!
//! ```text
//! cargo bench --bench expand                          # full local sweep
//! TTE_BENCH_MAX_ROWS=100000 cargo bench --bench expand -- --quick   # CI smoke
//! ```

#[path = "support.rs"]
mod support;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use polars::prelude::*;
use tte_expand::{Estimand, ExpandOptions, apply_weights, expand};

/// Periods per synthetic patient (mean follow-up before truncation ~13).
const PERIODS: u32 = 14;
/// Fixed PRNG seed — arbitrary but frozen, so every input is byte-identical
/// across runs.
const SEED: u64 = 0x0050_0005;

/// Target *input* row counts, filtered by `TTE_BENCH_MAX_ROWS`. Reading an env
/// var here is benchmark configuration, not part of the engine's transform path,
/// so it does not violate the determinism rule (the inputs themselves are
/// seed-deterministic regardless of which sizes are selected).
fn target_rows() -> Vec<usize> {
    let max = std::env::var("TTE_BENCH_MAX_ROWS")
        .ok()
        .and_then(|s| s.parse::<usize>().ok())
        .unwrap_or(1_000_000);
    [1_000usize, 10_000, 100_000, 1_000_000]
        .into_iter()
        .filter(|&n| n <= max)
        .collect()
}

fn opts(estimand: Estimand) -> ExpandOptions {
    ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand)
}

fn make_input(rows: usize) -> DataFrame {
    let patients = ((rows as u32) / PERIODS).max(1);
    support::gen_input_df(patients, PERIODS, SEED)
}

fn bench_estimand(c: &mut Criterion, label: &str, estimand: Estimand) {
    let mut group = c.benchmark_group(label);
    for rows in target_rows() {
        let df = make_input(rows);
        let n = df.height();
        let base = df.lazy();
        let options = opts(estimand);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                expand(base.clone(), &options)
                    .expect("expand")
                    .collect()
                    .expect("collect")
            });
        });
    }
    group.finish();
}

fn bench_itt(c: &mut Criterion) {
    bench_estimand(c, "expand_itt", Estimand::Itt);
}

fn bench_pp(c: &mut Criterion) {
    bench_estimand(c, "expand_pp", Estimand::PerProtocol);
}

fn bench_weighted(c: &mut Criterion) {
    let mut group = c.benchmark_group("expand_weighted_pp");
    for rows in target_rows() {
        let df = make_input(rows);
        let n = df.height();
        let factors = support::gen_factor_df(&df, SEED ^ 0xF).lazy();
        let base = df.lazy();
        let options = opts(Estimand::PerProtocol);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, _| {
            b.iter(|| {
                let expanded = expand(base.clone(), &options).expect("expand");
                apply_weights(expanded, factors.clone(), &options)
                    .expect("apply_weights")
                    .collect()
                    .expect("collect")
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_itt, bench_pp, bench_weighted);
criterion_main!(benches);
