//! Phase-6 contract test — **fitted** inverse-probability weights.
//!
//! STAGING: the agent guard blocks writing under `tests/`. A human moves this
//! into place with `git mv crates/tte-expand/tests-staging/weights_fit.rs
//! crates/tte-expand/tests/weights_fit.rs` (exactly how Phase 4 shipped its
//! testthat suite). It then runs as the canonical Phase-6 contract test.
//!
//! Unlike `tests/weights.rs` (Phase 3), which *applies* a pre-computed factor
//! table, this *fits* the IPW switch/censor models in Rust (binding `smartcore`)
//! and asserts the resulting weighted frame matches the Oracle: the six
//! structural columns **bit-exact**, and `weight` within the staged ~1e-6
//! tolerance (ADR-2). The fit converges to R `glm`'s MLE, not bit-for-bit.
//!
//! Requires the `weights-fit` feature (root CI runs `--all-features`); without
//! it the whole file compiles to nothing.
#![cfg(feature = "weights-fit")]
// Tolerances live in the harness, never in `src/`. Integration-crate helpers are
// not covered by `clippy.toml`'s allow-*-in-tests, so allow the test-only lints.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use polars::prelude::*;
use tte_expand::{
    CensorWeightSpec, Estimand, ExpandOptions, PoolCensor, SwitchWeightSpec, WeightSpec,
    expand_weighted_fitted_parquet, fit_weights,
};

/// Staged tolerance on the fitted `weight` (ADR-2). Observed worst on the
/// fixtures is ~3.4e-7 (solver-vs-`glm` ~1.6e-8 propagated through the cumulative
/// product); 1e-6 is the documented contract bound, defined here, not in `src/`.
const FITTED_WEIGHT_REL_TOL: f64 = 1e-6;

fn fixture(rel: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join(rel)
}

fn read_parquet(path: &Path) -> DataFrame {
    let s = path.to_str().expect("fixture path is valid UTF-8");
    LazyFrame::scan_parquet(PlRefPath::new(s), ScanArgsParquet::default())
        .expect("scan parquet")
        .collect()
        .expect("collect parquet")
}

fn weight_col(df: &DataFrame) -> Vec<f64> {
    df.column("weight")
        .expect("weight column")
        .f64()
        .expect("f64 weight")
        .into_no_null_iter()
        .collect()
}

fn options(estimand: Estimand) -> ExpandOptions {
    ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand)
}

/// The PP switching spec every switching scenario shares (`n = ~x2`, `d = ~x2 + x1`).
fn pp_switch_spec() -> WeightSpec {
    WeightSpec::switching(SwitchWeightSpec::new(["x2"], ["x2", "x1"]))
}

/// Fit `input_rel`'s weights under `spec`, apply them, and assert the result
/// equals `expected_rel`: structural columns bit-exact, `weight` within tolerance.
fn assert_fitted(input_rel: &str, expected_rel: &str, estimand: Estimand, spec: &WeightSpec) {
    let input = fixture(input_rel);
    let expected_path = fixture(expected_rel);
    for p in [&input, &expected_path] {
        assert!(p.exists(), "missing fixture: {}", p.display());
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join(format!("fitted_{}.parquet", expected_rel.replace('/', "_")));
    let opts = options(estimand);
    expand_weighted_fitted_parquet(&input, &out, &opts, spec).expect("fit + weighted expansion");

    let actual = read_parquet(&out);
    let expected = read_parquet(&expected_path);

    assert_eq!(
        actual.get_column_names(),
        expected.get_column_names(),
        "[{expected_rel}] column names/order differ"
    );
    assert_eq!(
        actual.dtypes(),
        expected.dtypes(),
        "[{expected_rel}] dtypes differ"
    );
    assert_eq!(
        actual.height(),
        expected.height(),
        "[{expected_rel}] row count differs"
    );

    for name in [
        "id",
        "trial_period",
        "followup_time",
        "assigned_treatment",
        "treatment",
        "outcome",
    ] {
        let a = actual.column(name).expect("actual column");
        let e = expected.column(name).expect("expected column");
        assert!(
            a.equals(e),
            "[{expected_rel}] structural column '{name}' differs\n--- actual (head 20) ---\n{}\n--- expected (head 20) ---\n{}",
            actual.head(Some(20)),
            expected.head(Some(20))
        );
    }

    let (aw, ew) = (weight_col(&actual), weight_col(&expected));
    let mut worst_rel = 0.0_f64;
    let mut worst_idx = 0usize;
    for (i, (a, e)) in aw.iter().zip(ew.iter()).enumerate() {
        let rel = (a - e).abs() / e.abs().max(1.0);
        if rel > worst_rel {
            worst_rel = rel;
            worst_idx = i;
        }
    }
    assert!(
        worst_rel <= FITTED_WEIGHT_REL_TOL,
        "[{expected_rel}] fitted weight exceeds tol {FITTED_WEIGHT_REL_TOL:e}: worst rel \
         {worst_rel:e} at row {worst_idx} (actual {}, expected {})",
        aw.get(worst_idx).copied().unwrap_or(f64::NAN),
        ew.get(worst_idx).copied().unwrap_or(f64::NAN)
    );
}

// ---- Per-protocol switching weights on committed scenario inputs. ----

#[test]
fn fitted_pp_switch_high_switching() {
    assert_fitted(
        "scenarios/input_high_switching.parquet",
        "weights/expected_high_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
        &pp_switch_spec(),
    );
}

#[test]
fn fitted_pp_switch_moderate_switching() {
    assert_fitted(
        "scenarios/input_moderate_switching.parquet",
        "weights/expected_moderate_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
        &pp_switch_spec(),
    );
}

#[test]
fn fitted_pp_switch_frequent_switching() {
    assert_fitted(
        "scenarios/input_frequent_switching.parquet",
        "weights/expected_frequent_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
        &pp_switch_spec(),
    );
}

// ---- IPCW + combined on the bundled `data_censored` cohort. ----

#[test]
fn fitted_pp_combined_data_censored() {
    let spec = pp_switch_spec().with_censor(CensorWeightSpec::new(
        "censored",
        ["x2"],
        ["x2", "x1"],
        PoolCensor::None,
    ));
    assert_fitted(
        "weights/input_data_censored.parquet",
        "weights/expected_data_censored_pp_weighted.parquet",
        Estimand::PerProtocol,
        &spec,
    );
}

#[test]
fn fitted_itt_ipcw_data_censored() {
    let spec = WeightSpec::ipcw(CensorWeightSpec::new(
        "censored",
        ["x2"],
        ["x2"],
        PoolCensor::Numerator,
    ));
    assert_fitted(
        "weights/input_data_censored.parquet",
        "weights/expected_data_censored_itt_weighted.parquet",
        Estimand::Itt,
        &spec,
    );
}

/// Determinism contract: the fit must be reproducible bit-for-bit run-to-run
/// (`smartcore`'s L-BFGS starts from a zero vector, no RNG), so two fits of the
/// same cohort produce byte-identical factor tables.
#[test]
fn fitted_weights_are_deterministic() {
    let input = fixture("weights/input_data_censored.parquet");
    let s = input.to_str().expect("utf8");
    let scan =
        || LazyFrame::scan_parquet(PlRefPath::new(s), ScanArgsParquet::default()).expect("scan");
    let spec = pp_switch_spec().with_censor(CensorWeightSpec::new(
        "censored",
        ["x2"],
        ["x2", "x1"],
        PoolCensor::None,
    ));
    let opts = options(Estimand::PerProtocol);
    let a = fit_weights(scan(), &opts, &spec)
        .expect("fit a")
        .collect()
        .expect("collect a");
    let b = fit_weights(scan(), &opts, &spec)
        .expect("fit b")
        .collect()
        .expect("collect b");
    assert!(a.equals(&b), "weight fit is not run-to-run deterministic");
}
