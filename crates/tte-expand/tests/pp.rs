//! Per-protocol (PP) artificial-censoring contract test against the R Oracle
//! fixtures. The engine (`tte_expand::expand_parquet` with
//! `Estimand::PerProtocol`) must reproduce the `TrialEmulation` per-protocol
//! expansion **bit-for-bit** on the six structural columns
//! (`id, trial_period, followup_time, assigned_treatment, treatment, outcome`),
//! including row count, row order, and per-column dtypes.
//!
//! PP is exactly the ITT expansion with each trial's follow-up censored at the
//! first period where `treatment` deviates from `assigned_treatment`: the
//! deviating row is dropped and a later switch-back never resumes follow-up.
//! It therefore carries the **same six columns/dtypes** as ITT — censoring shows
//! up purely as missing rows. `assigned_treatment` is retained (it equals
//! `treatment` on every surviving PP row by construction).
//!
//! Fixture naming follows the Oracle harness (`oracle/40_dump_fixtures.R`):
//! `input_<case>.parquet` and `expected_<case>_pp.parquet`, under repo-root
//! `fixtures/<subdir>/`. Cases are ordered by graded difficulty. PP == ITT for
//! the control-only / single-row cases (E01, E03, E05, E07, E08, E09); the
//! divergence cases are E02, E04, E06 and every simulated scenario.

// Test scaffolding: the fixture helpers below assert via `expect()`. clippy.toml
// already sets `allow-expect-in-tests`, but that only covers `#[test]` fns and
// `#[cfg(test)]` modules — not an integration crate's free helper fns — so allow
// it explicitly for this file.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use polars::prelude::*;
use tte_expand::{Estimand, ExpandOptions, expand_parquet};

/// Resolve a path under the repo-root `fixtures/` directory.
fn fixture(rel: &str) -> PathBuf {
    // crates/tte-expand -> repo root is two levels up.
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

/// Expand `input_<name>.parquet` under the per-protocol estimand via the public
/// `expand_parquet` round-trip and assert the result equals
/// `expected_<name>_pp.parquet` exactly.
fn assert_pp_fixture(subdir: &str, name: &str) {
    let input = fixture(&format!("{subdir}/input_{name}.parquet"));
    let expected_path = fixture(&format!("{subdir}/expected_{name}_pp.parquet"));
    assert!(input.exists(), "missing input fixture: {}", input.display());
    assert!(
        expected_path.exists(),
        "missing expected fixture: {}",
        expected_path.display()
    );

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("pp_{name}.parquet"));
    let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX)
        .with_estimand(Estimand::PerProtocol);
    expand_parquet(&input, &out, &opts).expect("expansion should succeed");

    let actual = read_parquet(&out);
    let expected = read_parquet(&expected_path);

    // Schema: column names AND dtypes, in order — bit-exactness lives here.
    assert_eq!(
        actual.get_column_names(),
        expected.get_column_names(),
        "[{name}] column names/order differ"
    );
    assert_eq!(
        actual.dtypes(),
        expected.dtypes(),
        "[{name}] dtypes differ\n  actual:   {:?}\n  expected: {:?}",
        actual.dtypes(),
        expected.dtypes()
    );
    assert_eq!(
        actual.height(),
        expected.height(),
        "[{name}] row count differs: actual {} vs expected {}",
        actual.height(),
        expected.height()
    );

    // Values: per-column exact equality with a readable row-level diff.
    for col_name in expected.get_column_names() {
        let a = actual.column(col_name).expect("actual column");
        let e = expected.column(col_name).expect("expected column");
        assert!(
            a.equals(e),
            "[{name}] column '{col_name}' differs\n--- actual (head 20) ---\n{}\n--- expected (head 20) ---\n{}",
            actual.head(Some(20)),
            expected.head(Some(20))
        );
    }
}

// ---- Edge battery (graded difficulty). ----

// E01/E03/E05/E07/E08/E09 never deviate (control-only / single-row), so PP == ITT
// — but the engine must still produce and match their PP fixtures exactly.
#[test]
fn e01_single_patient_single_period() {
    assert_pp_fixture("edge", "E01_single");
}

// E02 canonical: ITT 27 rows -> PP 11. Control trials (assigned=0) are censored
// when the patient initiates; the treated trial (assigned=1) runs to the end.
#[test]
fn e02_id4_canonical_divergence() {
    assert_pp_fixture("edge", "E02_id4_canonical");
}

#[test]
fn e03_event_at_baseline() {
    assert_pp_fixture("edge", "E03_event_at_baseline");
}

// E04 re-entry: ITT 11 rows -> PP 7. Control trials 0/1 censor at initiation; the
// re-entry trial (trial_period 3, assigned=1) never deviates.
#[test]
fn e04_reentry_divergence() {
    assert_pp_fixture("edge", "E04_reentry");
}

#[test]
fn e05_never_treats() {
    assert_pp_fixture("edge", "E05_never_treats");
}

// E06 switch-then-back (the canonical trap): treatment 1,1,0,1 with assigned=1.
// PP censors at the first deviation (followup_time 2) and does NOT resume at the
// switch-back (followup_time 3) — exactly followup_time 0 and 1 survive (2 rows).
#[test]
fn e06_switch_then_back() {
    assert_pp_fixture("edge", "E06_switch_then_back");
}

#[test]
fn e07_last_period_only() {
    assert_pp_fixture("edge", "E07_last_period_only");
}

#[test]
fn e08_ties() {
    assert_pp_fixture("edge", "E08_ties");
}

#[test]
fn e09_max_fanout() {
    assert_pp_fixture("edge", "E09_max_fanout");
}

// ---- Simulated scenario cohorts (events + censoring + switching). Every
//      scenario diverges from ITT (PP keeps a strict subset of the rows). ----

#[test]
fn scenario_common() {
    assert_pp_fixture("scenarios", "common");
}

#[test]
fn scenario_rare_event() {
    assert_pp_fixture("scenarios", "rare_event");
}

#[test]
fn scenario_ultra_rare_event() {
    assert_pp_fixture("scenarios", "ultra_rare_event");
}

#[test]
fn scenario_rare_initiation() {
    assert_pp_fixture("scenarios", "rare_initiation");
}

#[test]
fn scenario_high_switching() {
    assert_pp_fixture("scenarios", "high_switching");
}

#[test]
fn scenario_heavy_censoring() {
    assert_pp_fixture("scenarios", "heavy_censoring");
}

#[test]
fn scenario_short_followup() {
    assert_pp_fixture("scenarios", "short_followup");
}

#[test]
fn scenario_strong_confounding() {
    assert_pp_fixture("scenarios", "strong_confounding");
}
