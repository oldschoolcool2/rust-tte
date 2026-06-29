//! Intention-to-treat (ITT) expansion contract test against the R Oracle
//! fixtures. The engine (`tte_expand::expand_parquet`) must reproduce the
//! `TrialEmulation` expansion **bit-for-bit** on the six structural columns
//! (`id, trial_period, followup_time, assigned_treatment, treatment, outcome`),
//! including row count, row order, and per-column dtypes.
//!
//! Fixture naming follows the Oracle harness (`oracle/40_dump_fixtures.R`):
//! `input_<case>.parquet` and `expected_<case>_itt.parquet`, under repo-root
//! `fixtures/<subdir>/`. Cases are ordered by graded difficulty (single →
//! multi-trial → baseline event → never-treats → last-period → scenario cohorts).

// Test scaffolding: the fixture helpers below assert via `expect()`. clippy.toml
// already sets `allow-expect-in-tests`, but that only covers `#[test]` fns and
// `#[cfg(test)]` modules — not an integration crate's free helper fns — so allow
// it explicitly for this file.
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::path::{Path, PathBuf};

use polars::prelude::*;
use tte_expand::{ExpandOptions, expand_parquet};

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

/// Expand `input_<name>.parquet` via the public `expand_parquet` round-trip and
/// assert the result equals `expected_<name>_itt.parquet` exactly.
fn assert_itt_fixture(subdir: &str, name: &str) {
    let input = fixture(&format!("{subdir}/input_{name}.parquet"));
    let expected_path = fixture(&format!("{subdir}/expected_{name}_itt.parquet"));
    assert!(input.exists(), "missing input fixture: {}", input.display());
    assert!(
        expected_path.exists(),
        "missing expected fixture: {}",
        expected_path.display()
    );

    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("itt_{name}.parquet"));
    let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
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

#[test]
fn e01_single_patient_single_period() {
    assert_itt_fixture("edge", "E01_single");
}

#[test]
fn e02_id4_canonical_multi_trial() {
    assert_itt_fixture("edge", "E02_id4_canonical");
}

#[test]
fn e03_event_at_baseline() {
    assert_itt_fixture("edge", "E03_event_at_baseline");
}

#[test]
fn e05_never_treats() {
    assert_itt_fixture("edge", "E05_never_treats");
}

#[test]
fn e07_last_period_only() {
    assert_itt_fixture("edge", "E07_last_period_only");
}

// E04 re-entry: a re-entered eligible period seeds a new trial whose
// assigned_treatment comes from the re-entry period (trials 0,1,3; assigned 0,0,1).
#[test]
fn e04_reentry() {
    assert_itt_fixture("edge", "E04_reentry");
}

// E06 switch-then-back: ITT carries the 1->0->1 trajectory with no censoring
// (assigned=1 frozen; treatment column 1,1,0,1). PP censoring is Phase 2.
#[test]
fn e06_switch_then_back() {
    assert_itt_fixture("edge", "E06_switch_then_back");
}

// E08 ties: a terminal event recorded across all three overlapping trials.
#[test]
fn e08_ties() {
    assert_itt_fixture("edge", "E08_ties");
}

// E09 max fan-out: 31 eligible periods -> 496 rows (31*32/2), the row-count invariant.
#[test]
fn e09_max_fanout() {
    assert_itt_fixture("edge", "E09_max_fanout");
}

// ---- Simulated scenario cohorts (events + censoring + switching). ----

#[test]
fn scenario_common() {
    assert_itt_fixture("scenarios", "common");
}

#[test]
fn scenario_rare_event() {
    assert_itt_fixture("scenarios", "rare_event");
}

#[test]
fn scenario_ultra_rare_event() {
    assert_itt_fixture("scenarios", "ultra_rare_event");
}

#[test]
fn scenario_rare_initiation() {
    assert_itt_fixture("scenarios", "rare_initiation");
}

#[test]
fn scenario_high_switching() {
    assert_itt_fixture("scenarios", "high_switching");
}

#[test]
fn scenario_heavy_censoring() {
    assert_itt_fixture("scenarios", "heavy_censoring");
}

#[test]
fn scenario_short_followup() {
    assert_itt_fixture("scenarios", "short_followup");
}

#[test]
fn scenario_strong_confounding() {
    assert_itt_fixture("scenarios", "strong_confounding");
}
