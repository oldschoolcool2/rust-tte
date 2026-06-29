//! Intention-to-treat (ITT) expansion contract test against the R Oracle
//! fixtures. This is currently a SKELETON: it is `#[ignore]`d because
//! `tte_expand::expand_parquet` is an unimplemented stub. Remove `#[ignore]`
//! once the engine lands.
//!
//! Fixture naming follows the Oracle harness (`oracle/40_dump_fixtures.R`):
//! `input_<case>.parquet` and `expected_<case>_<estimand>.parquet`, written
//! under repo-root `fixtures/<subdir>/`. The case below is the floor edge case
//! `E01_single` (one eligible patient, one period) from `oracle/30_edge_cases.R`.

use std::path::{Path, PathBuf};

use tte_expand::{ExpandOptions, expand_parquet};

/// Resolve a path under the repo-root `fixtures/` directory.
fn fixture(rel: &str) -> PathBuf {
    // crates/tte-expand -> repo root is two levels up.
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join(rel)
}

#[test]
#[ignore = "expansion engine not implemented yet; remove once tte_expand::expand_parquet lands"]
fn itt_expansion_matches_oracle() {
    let input = fixture("edge/input_E01_single.parquet");
    let expected = fixture("edge/expected_E01_single_itt.parquet");

    assert!(input.exists(), "missing input fixture: {}", input.display());
    assert!(
        expected.exists(),
        "missing expected fixture: {}",
        expected.display()
    );

    let out_dir = Path::new(env!("CARGO_TARGET_TMPDIR"));
    let out = out_dir.join("itt_actual.parquet");

    let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
    expand_parquet(&input, &out, &opts).expect("expansion should succeed");

    // TODO(engine): load `out` and `expected` with Polars and assert a
    // dtype-exact frame equality (schema + values + categorical mapping) on the
    // structural columns id/trial_period/followup_time/assigned_treatment/
    // treatment/outcome once the expansion engine is implemented.
}
