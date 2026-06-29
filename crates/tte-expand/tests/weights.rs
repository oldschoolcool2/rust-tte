//! Weight-application contract test against the R Oracle fixtures (Phase 3).
//!
//! The engine reproduces `TrialEmulation`'s *weighted* expanded frame by joining
//! the pre-computed per-`(id, period)` IPW factor onto the structural expansion
//! and taking the cumulative product within each `(id, trial_period)` ordered by
//! `followup_time` (SPEC §4). The six structural columns
//! (`id, trial_period, followup_time, assigned_treatment, treatment, outcome`)
//! match **exactly** (schema + values + order + row count); the `weight` column
//! (Float64) matches within a small float tolerance — the engine redoes the
//! cumulative product and may reassociate relative to R (ADR-2).
//!
//! Fixtures (`oracle/42_dump_weights.R`), under repo-root `fixtures/weights/`:
//!
//! - `input_<name>_<estimand>_weights.parquet` — per-(id,period) factor table
//!   (`id, period, weight_factor`) the engine joins.
//! - `expected_<name>_<estimand>_weighted.parquet` — the target weighted frame.
//!
//! Structural inputs the engine expands: committed `fixtures/scenarios/input_*`
//! for the scenario cases; `fixtures/weights/input_data_censored.parquet` for the
//! bundled-cohort cases.

// Tolerances live in the harness, never in `src/`. Free helper fns in an
// integration crate aren't covered by clippy.toml's allow-*-in-tests, so allow
// the test-only lints explicitly here.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use polars::prelude::*;
use tte_expand::{Estimand, ExpandOptions, expand_weighted_parquet};

/// Harness tolerance on the float `weight` (ADR-2: exact structure, ~1e-12 on the
/// product). Observed engine-vs-Oracle reassociation is ~1e-15; this is the
/// contract bound, defined here and nowhere in `src/`.
const WEIGHT_REL_TOL: f64 = 1e-12;

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

fn f64_col(df: &DataFrame, name: &str) -> Vec<f64> {
    df.column(name)
        .expect("column")
        .cast(&DataType::Float64)
        .expect("cast f64")
        .f64()
        .expect("f64")
        .into_no_null_iter()
        .collect()
}

/// Expand `input_rel` under `estimand`, apply the Oracle factor table
/// `factors_rel`, and assert the result equals `expected_rel`: structural columns
/// exact, `weight` within `WEIGHT_REL_TOL`.
fn assert_weighted(input_rel: &str, factors_rel: &str, expected_rel: &str, estimand: Estimand) {
    let input = fixture(input_rel);
    let factors = fixture(factors_rel);
    let expected_path = fixture(expected_rel);
    for p in [&input, &factors, &expected_path] {
        assert!(p.exists(), "missing fixture: {}", p.display());
    }

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "weighted_{}.parquet",
        expected_rel.replace('/', "_")
    ));
    let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand);
    expand_weighted_parquet(&input, &factors, &out, &opts)
        .expect("weighted expansion should succeed");

    let actual = read_parquet(&out);
    let expected = read_parquet(&expected_path);

    // Schema: names + dtypes + order (the weighted schema is the 6 structural
    // columns followed by `weight`).
    assert_eq!(
        actual.get_column_names(),
        expected.get_column_names(),
        "[{expected_rel}] column names/order differ"
    );
    assert_eq!(
        actual.dtypes(),
        expected.dtypes(),
        "[{expected_rel}] dtypes differ\n  actual:   {:?}\n  expected: {:?}",
        actual.dtypes(),
        expected.dtypes()
    );
    assert_eq!(
        actual.height(),
        expected.height(),
        "[{expected_rel}] row count differs: {} vs {}",
        actual.height(),
        expected.height()
    );

    // Structural columns: exact equality, with a readable head-diff.
    for col_name in [
        "id",
        "trial_period",
        "followup_time",
        "assigned_treatment",
        "treatment",
        "outcome",
    ] {
        let a = actual.column(col_name).expect("actual column");
        let e = expected.column(col_name).expect("expected column");
        assert!(
            a.equals(e),
            "[{expected_rel}] structural column '{col_name}' differs\n--- actual (head 20) ---\n{}\n--- expected (head 20) ---\n{}",
            actual.head(Some(20)),
            expected.head(Some(20))
        );
    }

    // `weight`: relative tolerance, reporting the worst offending row.
    let aw = f64_col(&actual, "weight");
    let ew = f64_col(&expected, "weight");
    let mut worst_idx = 0usize;
    let mut worst_rel = 0.0f64;
    for (i, (a, e)) in aw.iter().zip(ew.iter()).enumerate() {
        let rel = (a - e).abs() / e.abs().max(1.0);
        if rel > worst_rel {
            worst_rel = rel;
            worst_idx = i;
        }
    }
    assert!(
        worst_rel <= WEIGHT_REL_TOL,
        "[{expected_rel}] weight exceeds tol {WEIGHT_REL_TOL:e}: worst rel {worst_rel:e} at row {worst_idx} (actual {}, expected {})",
        aw.get(worst_idx).copied().unwrap_or(f64::NAN),
        ew.get(worst_idx).copied().unwrap_or(f64::NAN)
    );
}

// ---- Per-protocol switching weights on committed scenario inputs. ----

#[test]
fn pp_switch_high_switching() {
    assert_weighted(
        "scenarios/input_high_switching.parquet",
        "weights/input_high_switching_pp_weights.parquet",
        "weights/expected_high_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
    );
}

#[test]
fn pp_switch_moderate_switching() {
    assert_weighted(
        "scenarios/input_moderate_switching.parquet",
        "weights/input_moderate_switching_pp_weights.parquet",
        "weights/expected_moderate_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
    );
}

#[test]
fn pp_switch_frequent_switching() {
    assert_weighted(
        "scenarios/input_frequent_switching.parquet",
        "weights/input_frequent_switching_pp_weights.parquet",
        "weights/expected_frequent_switching_pp_weighted.parquet",
        Estimand::PerProtocol,
    );
}

// ---- IPCW on the bundled `data_censored` cohort (explicit censoring). ----

// PP combining switching + censoring weights.
#[test]
fn pp_combined_data_censored() {
    assert_weighted(
        "weights/input_data_censored.parquet",
        "weights/input_data_censored_pp_weights.parquet",
        "weights/expected_data_censored_pp_weighted.parquet",
        Estimand::PerProtocol,
    );
}

// ITT inverse-probability-of-censoring weights applied to the un-censored frame.
#[test]
fn itt_ipcw_data_censored() {
    assert_weighted(
        "weights/input_data_censored.parquet",
        "weights/input_data_censored_itt_weights.parquet",
        "weights/expected_data_censored_itt_weighted.parquet",
        Estimand::Itt,
    );
}
