#![forbid(unsafe_code)]
//! # tte-expand
//!
//! Verified, high-performance backend for the **data-expansion** stage of
//! *sequential target trial emulation* (epidemiology). It reproduces, bit-for-bit,
//! the expansion output of the R package
//! [`TrialEmulation`](https://cran.r-project.org/package=TrialEmulation)
//! (Apache-2.0) using a Polars lazy engine with dtype-exact, deterministic
//! integer/float handling.
//!
//! Validation is fixture-driven: an R "Oracle" emits Parquet fixtures and this
//! crate must match them exactly (see `tests/itt.rs` and the in-crate fixture
//! tests). This crate is `#![forbid(unsafe_code)]`.
//!
//! ## ITT expansion in one paragraph
//!
//! The input is long person-time: one row per `(id, period)` carrying
//! `eligible`, `treatment` and `outcome`. For every `(id, period)` with
//! `eligible == 1` the engine seeds an emulated trial at `trial_period = period`
//! and emits a follow-up row for every later observed period of that patient.
//! On each follow-up row `followup_time = period - trial_period`, `treatment`
//! and `outcome` are the patient's *actual* values at that period, and
//! `assigned_treatment` is the patient's treatment at the trial baseline, carried
//! forward unchanged (intention-to-treat does **not** censor on switching).

use std::path::Path;

use polars::prelude::*;
use thiserror::Error;

/// Output column: the period at which an emulated trial starts.
const COL_TRIAL_PERIOD: &str = "trial_period";
/// Output column: 0-based offset within a trial (`period - trial_period`).
const COL_FOLLOWUP_TIME: &str = "followup_time";
/// Output column: treatment at the trial baseline, carried forward (ITT).
const COL_ASSIGNED_TREATMENT: &str = "assigned_treatment";

/// Errors returned by the expansion engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExpandError {
    /// A Polars query/IO operation failed.
    #[error("polars error: {0}")]
    Polars(#[from] PolarsError),
    /// A filesystem I/O operation failed.
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    /// The supplied [`ExpandOptions`] were invalid (e.g. `first_period > last_period`).
    #[error("invalid expansion options: {0}")]
    InvalidOptions(String),
}

/// Convenience alias for results produced by this crate.
pub type Result<T> = std::result::Result<T, ExpandError>;

/// Configuration for a single expansion run.
///
/// Construct via [`ExpandOptions::new`]; the struct is `#[non_exhaustive]` so
/// new fields can be added without a breaking change. The `eligible`/`outcome`
/// column names default to `"eligible"`/`"outcome"` (override with the builder
/// setters).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExpandOptions {
    /// Subject identifier column.
    pub id_col: String,
    /// Integer period (time-step) column.
    pub period_col: String,
    /// Treatment indicator column.
    pub treatment_col: String,
    /// Eligibility (`{0,1}`) column; `1` seeds a trial at that period.
    pub eligible_col: String,
    /// Outcome / event-indicator column.
    pub outcome_col: String,
    /// Inclusive lower bound on `trial_period` (trials seeded earlier are dropped).
    pub first_period: i32,
    /// Inclusive upper bound on `trial_period` (trials seeded later are dropped).
    pub last_period: i32,
}

impl ExpandOptions {
    /// Build a new [`ExpandOptions`] with the default `"eligible"`/`"outcome"`
    /// column names.
    #[must_use]
    pub fn new(
        id_col: &str,
        period_col: &str,
        treatment_col: &str,
        first_period: i32,
        last_period: i32,
    ) -> Self {
        Self {
            id_col: id_col.to_owned(),
            period_col: period_col.to_owned(),
            treatment_col: treatment_col.to_owned(),
            eligible_col: "eligible".to_owned(),
            outcome_col: "outcome".to_owned(),
            first_period,
            last_period,
        }
    }

    /// Override the eligibility column name.
    #[must_use]
    pub fn with_eligible_col(mut self, eligible_col: &str) -> Self {
        eligible_col.clone_into(&mut self.eligible_col);
        self
    }

    /// Override the outcome column name.
    #[must_use]
    pub fn with_outcome_col(mut self, outcome_col: &str) -> Self {
        outcome_col.clone_into(&mut self.outcome_col);
        self
    }
}

/// Expand a prepared person-time [`LazyFrame`] into the sequential
/// target-trial layout (intention-to-treat).
///
/// The returned frame carries exactly the six structural columns, in order:
/// `id, trial_period, followup_time, assigned_treatment, treatment, outcome`,
/// sorted by `(id, trial_period, followup_time)`.
///
/// # Dtypes
/// To match the Oracle bit-for-bit the engine preserves input dtypes and applies
/// the same coercions `TrialEmulation` does: `id`, `assigned_treatment` and
/// `treatment` pass the input dtype through; `trial_period` is `Int32`;
/// `followup_time` inherits the input `period` dtype; `outcome` is `Float64`.
///
/// # Errors
/// Returns [`ExpandError`] if `options` are invalid (`first_period > last_period`,
/// or the `period` column is absent) or a Polars operation fails.
pub fn expand(input: LazyFrame, options: &ExpandOptions) -> Result<LazyFrame> {
    if options.first_period > options.last_period {
        return Err(ExpandError::InvalidOptions(format!(
            "first_period ({}) must not exceed last_period ({})",
            options.first_period, options.last_period
        )));
    }

    let id = options.id_col.as_str();
    let period = options.period_col.as_str();
    let treatment = options.treatment_col.as_str();
    let eligible = options.eligible_col.as_str();
    let outcome = options.outcome_col.as_str();

    // `followup_time` must reproduce the input `period` column's dtype exactly
    // (Float64 stays Float64, Int32 stays Int32) — the Oracle derives it as
    // `period - trial_period`, inheriting `period`'s dtype.
    let period_dtype = {
        let mut probe = input.clone();
        let schema = probe.collect_schema()?;
        schema.get(period).cloned().ok_or_else(|| {
            ExpandError::InvalidOptions(format!("input is missing period column '{period}'"))
        })?
    };

    // Seeds: one row per eligible (id, period) whose period is in range; the
    // baseline treatment becomes the (carried-forward) assigned treatment.
    let seeds = input
        .clone()
        .filter(
            col(eligible)
                .eq(lit(1i32))
                .and(col(period).gt_eq(lit(options.first_period)))
                .and(col(period).lt_eq(lit(options.last_period))),
        )
        .select([
            col(id),
            col(period).cast(DataType::Int32).alias(COL_TRIAL_PERIOD),
            col(treatment).alias(COL_ASSIGNED_TREATMENT),
        ]);

    // Follow-up universe: every observed period with its actual treatment/outcome.
    let follow = input.select([col(id), col(period), col(treatment), col(outcome)]);

    let expanded = seeds
        .join(
            follow,
            [col(id)],
            [col(id)],
            JoinArgs::new(JoinType::Inner),
        )
        // Keep only follow-up at or after each trial's baseline period.
        .filter(col(period).gt_eq(col(COL_TRIAL_PERIOD)))
        .with_columns([
            (col(period) - col(COL_TRIAL_PERIOD))
                .cast(period_dtype)
                .alias(COL_FOLLOWUP_TIME),
            col(outcome).cast(DataType::Float64),
        ])
        .select([
            col(id),
            col(COL_TRIAL_PERIOD),
            col(COL_FOLLOWUP_TIME),
            col(COL_ASSIGNED_TREATMENT),
            col(treatment),
            col(outcome),
        ])
        // (id, trial_period, followup_time) is a unique key, so this total,
        // explicit, ascending sort is fully deterministic.
        .sort_by_exprs(
            [col(id), col(COL_TRIAL_PERIOD), col(COL_FOLLOWUP_TIME)],
            SortMultipleOptions::default()
                .with_order_descending(false)
                .with_maintain_order(true),
        );

    Ok(expanded)
}

/// Read the Parquet file at `input_path`, expand it (ITT), and write the
/// dtype-exact result to `output_path`.
///
/// # Errors
/// Returns [`ExpandError`] if reading/writing fails, the input path is not valid
/// UTF-8, `options` are invalid, or a Polars operation fails.
pub fn expand_parquet(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    options: &ExpandOptions,
) -> Result<()> {
    let path_str = input_path
        .as_ref()
        .to_str()
        .ok_or_else(|| ExpandError::InvalidOptions("input path is not valid UTF-8".to_owned()))?;
    let lf = LazyFrame::scan_parquet(PlRefPath::new(path_str), ScanArgsParquet::default())?;
    let mut frame = expand(lf, options)?.collect()?;
    let mut file = std::fs::File::create(output_path)?;
    ParquetWriter::new(&mut file).finish(&mut frame)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! In-crate fixture verification. These tests load the Oracle Parquet
    //! fixtures (repo-root `fixtures/`) and assert bit-exact frame equality on
    //! the structural columns. The canonical contract test lives in
    //! `tests/itt.rs`; this module is the engine's co-located regression net.
    use std::path::{Path, PathBuf};

    use super::{ExpandOptions, expand, expand_parquet};
    use polars::prelude::*;

    /// Resolve a path under the repo-root `fixtures/` directory.
    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures")
            .join(rel)
    }

    fn read_parquet(path: &Path) -> DataFrame {
        let s = path.to_str().expect("fixture path is valid UTF-8");
        LazyFrame::scan_parquet(PlRefPath::new(s), ScanArgsParquet::default())
            .expect("scan fixture")
            .collect()
            .expect("collect fixture")
    }

    /// Expand `fixtures/<subdir>/input_<name>.parquet` and assert it equals
    /// `fixtures/<subdir>/expected_<name>_itt.parquet` exactly (schema + values).
    fn assert_itt_matches(subdir: &str, name: &str) {
        let input = fixture(&format!("{subdir}/input_{name}.parquet"));
        let expected_path = fixture(&format!("{subdir}/expected_{name}_itt.parquet"));
        assert!(input.exists(), "missing input fixture: {}", input.display());
        assert!(
            expected_path.exists(),
            "missing expected fixture: {}",
            expected_path.display()
        );

        let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
        let lf = LazyFrame::scan_parquet(
            PlRefPath::new(input.to_str().expect("utf8")),
            ScanArgsParquet::default(),
        )
        .expect("scan input");
        let actual = expand(lf, &opts)
            .expect("expand ok")
            .collect()
            .expect("collect actual");
        let expected = read_parquet(&expected_path);

        // Schema: column names AND dtypes, in order (bit-exactness lives here).
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

        // Values: per-column exact equality with a readable frame-level diff.
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

    // ---- Edge battery (graded order: single -> multi-trial -> baseline event
    //      -> never-treats -> last-period). ----
    #[test]
    fn e01_single_patient_single_period() {
        assert_itt_matches("edge", "E01_single");
    }

    #[test]
    fn e02_id4_canonical_multi_trial() {
        assert_itt_matches("edge", "E02_id4_canonical");
    }

    #[test]
    fn e03_event_at_baseline() {
        assert_itt_matches("edge", "E03_event_at_baseline");
    }

    #[test]
    fn e05_never_treats_max_fanout() {
        assert_itt_matches("edge", "E05_never_treats");
    }

    #[test]
    fn e07_last_period_only_single_row_trial() {
        assert_itt_matches("edge", "E07_last_period_only");
    }

    #[test]
    fn e04_reentry_assignment_from_reentry_period() {
        assert_itt_matches("edge", "E04_reentry");
    }

    #[test]
    fn e06_switch_then_back_itt_no_censoring() {
        assert_itt_matches("edge", "E06_switch_then_back");
    }

    #[test]
    fn e08_ties_event_across_overlapping_trials() {
        assert_itt_matches("edge", "E08_ties");
    }

    #[test]
    fn e09_max_fanout_row_count_invariant() {
        assert_itt_matches("edge", "E09_max_fanout");
    }

    // ---- Simulated scenario cohorts (events + censoring + switching). ----
    #[test]
    fn scenario_common() {
        assert_itt_matches("scenarios", "common");
    }

    #[test]
    fn scenario_rare_event() {
        assert_itt_matches("scenarios", "rare_event");
    }

    #[test]
    fn scenario_ultra_rare_event() {
        assert_itt_matches("scenarios", "ultra_rare_event");
    }

    #[test]
    fn scenario_rare_initiation() {
        assert_itt_matches("scenarios", "rare_initiation");
    }

    #[test]
    fn scenario_high_switching() {
        assert_itt_matches("scenarios", "high_switching");
    }

    #[test]
    fn scenario_heavy_censoring() {
        assert_itt_matches("scenarios", "heavy_censoring");
    }

    #[test]
    fn scenario_short_followup() {
        assert_itt_matches("scenarios", "short_followup");
    }

    #[test]
    fn scenario_strong_confounding() {
        assert_itt_matches("scenarios", "strong_confounding");
    }

    // ---- Public `expand_parquet` round-trip (write -> reread dtype fidelity). ----
    #[test]
    fn expand_parquet_roundtrip_matches_oracle() {
        let tmp = std::env::temp_dir();
        for (subdir, name) in [("edge", "E02_id4_canonical"), ("scenarios", "common")] {
            let input = fixture(&format!("{subdir}/input_{name}.parquet"));
            let expected_path = fixture(&format!("{subdir}/expected_{name}_itt.parquet"));
            let out = tmp.join(format!("tte_rt_{name}.parquet"));
            let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
            expand_parquet(&input, &out, &opts).expect("expand_parquet ok");

            let actual = read_parquet(&out);
            let expected = read_parquet(&expected_path);
            assert_eq!(
                actual.get_column_names(),
                expected.get_column_names(),
                "[{name}] names"
            );
            assert_eq!(actual.dtypes(), expected.dtypes(), "[{name}] dtypes");
            assert_eq!(actual.height(), expected.height(), "[{name}] height");
            for c in expected.get_column_names() {
                let a = actual.column(c).expect("actual col");
                let e = expected.column(c).expect("expected col");
                assert!(
                    a.equals(e),
                    "[{name}] column '{c}' differs after round-trip"
                );
            }
        }
    }

    // ---- Invariants (property-style checks on the canonical multi-trial case). ----
    #[test]
    fn invariant_one_baseline_row_per_trial() {
        let input = fixture("edge/input_E02_id4_canonical.parquet");
        let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
        let lf = LazyFrame::scan_parquet(
            PlRefPath::new(input.to_str().expect("utf8")),
            ScanArgsParquet::default(),
        )
        .expect("scan");
        let out = expand(lf, &opts)
            .expect("expand")
            .collect()
            .expect("collect");

        // followup_time >= 0 everywhere.
        let min_fu = out
            .column("followup_time")
            .expect("col")
            .cast(&DataType::Int64)
            .expect("cast")
            .i64()
            .expect("i64")
            .min()
            .expect("min");
        assert!(
            min_fu >= 0,
            "followup_time must be non-negative, got {min_fu}"
        );

        // Exactly one followup_time==0 baseline row per (id, trial_period): there
        // are 3 trials (trial_period 0,1,2) for id=4, hence 3 baseline rows.
        let baselines = out
            .lazy()
            .filter(col("followup_time").cast(DataType::Int64).eq(lit(0i64)))
            .collect()
            .expect("filter baselines");
        assert_eq!(baselines.height(), 3, "expected one baseline per trial");
    }

    #[test]
    fn invariant_assigned_treatment_sourced_from_trial_period() {
        // SPEC §2 invariant (the re-entry-critical property): assigned_treatment
        // within (id, trial_period) is the patient's treatment AT trial_period, not
        // frozen from first eligibility. E04 re-entry (trials 0,1,3) carries assigned
        // 0,0,1; a freeze-from-first-eligibility bug would wrongly give trial 3 = 0.
        let input = fixture("edge/input_E04_reentry.parquet");
        let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
        let lf = LazyFrame::scan_parquet(
            PlRefPath::new(input.to_str().expect("utf8")),
            ScanArgsParquet::default(),
        )
        .expect("scan");
        let out = expand(lf, &opts)
            .expect("expand")
            .collect()
            .expect("collect");

        // Baseline treatment per period, straight from the input.
        let baseline = read_parquet(&input).lazy().select([
            col("period").cast(DataType::Int64).alias("tp_key"),
            col("treatment")
                .cast(DataType::Int64)
                .alias("baseline_treatment"),
        ]);

        // Join every output row to the input baseline at period == trial_period;
        // assigned_treatment must never disagree.
        let mismatches = out
            .clone()
            .lazy()
            .with_columns([
                col("trial_period").cast(DataType::Int64).alias("tp_key"),
                col("assigned_treatment")
                    .cast(DataType::Int64)
                    .alias("assigned_i"),
            ])
            .join(
                baseline,
                [col("tp_key")],
                [col("tp_key")],
                JoinArgs::new(JoinType::Inner),
            )
            .filter(col("assigned_i").neq(col("baseline_treatment")))
            .collect()
            .expect("join");
        assert_eq!(
            mismatches.height(),
            0,
            "assigned_treatment must equal input treatment at trial_period for every row"
        );

        // Concretely: the re-entry trial (trial_period == 3) carries assigned == 1.
        let reentry = out
            .lazy()
            .filter(col("trial_period").cast(DataType::Int64).eq(lit(3i64)))
            .filter(
                col("assigned_treatment")
                    .cast(DataType::Int64)
                    .eq(lit(1i64)),
            )
            .collect()
            .expect("reentry");
        assert_eq!(
            reentry.height(),
            2,
            "E04 re-entry trial (3) must carry assigned=1 across its 2 rows"
        );
    }
}
