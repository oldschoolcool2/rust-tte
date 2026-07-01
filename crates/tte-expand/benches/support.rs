//! Shared, dependency-free, deterministic benchmark-input generator.
//!
//! Not a bench target (the crate sets `autobenches = false`): the bench files
//! pull it in with `#[path = "support.rs"] mod support;`. Inputs are produced
//! from a fixed seed with a self-contained PRNG, so every run / machine / core
//! count yields byte-identical frames (the determinism rule applies to the
//! benchmark inputs too, not just the engine).
#![allow(
    // Included as a private `mod` by each bench; its `pub` API is crate-internal.
    unreachable_pub,
    // Synthetic-data generation: the numeric-exactness lints that guard `src/`
    // are not meaningful for a throwaway benchmark generator.
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::missing_panics_doc
)]

use polars::prelude::*;

/// `SplitMix64` — a tiny deterministic PRNG (no dependency, no global / OS state).
/// Identical output for a given seed regardless of platform or thread count.
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    #[inline]
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniform `f64` in `[0, 1)` from the top 53 bits.
    #[inline]
    pub fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Generate a well-formed long person-time input frame of up to
/// `patients * periods` rows (fewer after first-outcome truncation), in the
/// scenario-fixture schema: `id, period, eligible, treatment` Int32, `x1`
/// Float64, `x2` / `outcome` Int32 — the same dtypes the engine's scenario
/// fixtures carry, so it exercises the real expansion path.
///
/// Semantics mirror `oracle/10_simulate.R` in vectorized form: absorbing
/// treatment from a per-patient initiation period, recurrent eligibility while
/// naive (so one patient seeds many trials), and a rare outcome that truncates
/// the patient's rows — giving the realistic ~3-5x ITT fan-out the engine is
/// benchmarked on. Fully deterministic in `seed`.
pub fn gen_input_df(patients: u32, periods: u32, seed: u64) -> DataFrame {
    let mut rng = SplitMix64::new(seed);
    let cap = (patients as usize).saturating_mul(periods as usize);
    let mut id = Vec::with_capacity(cap);
    let mut period = Vec::with_capacity(cap);
    let mut eligible = Vec::with_capacity(cap);
    let mut treatment = Vec::with_capacity(cap);
    let mut x1 = Vec::with_capacity(cap);
    let mut x2 = Vec::with_capacity(cap);
    let mut outcome = Vec::with_capacity(cap);

    let haz = 0.22_f64; // per-period initiation hazard (geometric init period)
    let p_out = 0.006_f64; // rare outcome
    let p_never = 0.10_f64; // share that never initiates -> max fan-out trials

    for pid in 0..patients {
        // Absorbing initiation period (geometric), or "never" for a slice.
        let init = if rng.unit() < p_never {
            periods
        } else {
            let mut k = 0u32;
            while k < periods && rng.unit() >= haz {
                k += 1;
            }
            k
        };
        for t in 0..periods {
            let elig = i32::from(t <= init); // naive at start of period -> eligible
            let trt = i32::from(t >= init); // absorbing treatment from `init`
            let y = i32::from(rng.unit() < p_out);
            id.push(pid as i32);
            period.push(t as i32);
            eligible.push(elig);
            treatment.push(trt);
            x1.push(rng.unit().mul_add(2.0, -1.0)); // unused by `expand`; present for schema
            x2.push(i32::from(rng.unit() < 0.5));
            outcome.push(y);
            if y == 1 {
                break; // truncate at first outcome -> per-id periods stay contiguous 0..t
            }
        }
    }

    df!(
        "id" => id,
        "period" => period,
        "eligible" => eligible,
        "treatment" => treatment,
        "x1" => x1,
        "x2" => x2,
        "outcome" => outcome,
    )
    .expect("synthetic input frame is well-formed")
}

/// Build a per-`(id, period)` weight-factor table (`id`, `period` Int32;
/// `weight_factor` Float64) covering every `(id, period)` in `input`, with a
/// deterministic factor in `[0.5, 1.5)`. Weight *values* are irrelevant to
/// timing; this only exercises the join + cumulative-product path of
/// [`apply_weights`](tte_expand::apply_weights).
pub fn gen_factor_df(input: &DataFrame, seed: u64) -> DataFrame {
    let id = input.column("id").expect("id").i32().expect("i32 id");
    let period = input
        .column("period")
        .expect("period")
        .i32()
        .expect("i32 period");
    let mut rng = SplitMix64::new(seed);
    let n = input.height();
    let mut fid = Vec::with_capacity(n);
    let mut fperiod = Vec::with_capacity(n);
    let mut factor = Vec::with_capacity(n);
    for i in 0..n {
        fid.push(id.get(i).expect("id value"));
        fperiod.push(period.get(i).expect("period value"));
        factor.push(0.5 + rng.unit());
    }
    df!(
        "id" => fid,
        "period" => fperiod,
        "weight_factor" => factor,
    )
    .expect("synthetic factor frame is well-formed")
}
