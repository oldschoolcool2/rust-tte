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
/// Output column (Phase 3): the per-row inverse-probability weight.
const COL_WEIGHT: &str = "weight";
/// Join column: each follow-up row's calendar period (`trial_period +
/// followup_time`), used to attach the per-`(id, period)` weight factor.
const COL_PERIOD: &str = "period";
/// Factor-table column: the pre-computed per-`(id, period)` IPW multiplier the
/// engine joins and then accumulates (see [`apply_weights`]).
const COL_WEIGHT_FACTOR: &str = "weight_factor";

/// Causal estimand selecting whether follow-up is artificially censored at the
/// first treatment deviation.
///
/// The variant controls only the *structural* censoring (which rows survive);
/// no statistics are involved. Defaults to [`Estimand::Itt`] so existing callers
/// are unaffected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[non_exhaustive]
pub enum Estimand {
    /// Intention-to-treat: no artificial censoring. Follow-up runs to the
    /// patient's last observed period regardless of treatment switching.
    #[default]
    Itt,
    /// Per-protocol: censor each emulated trial's follow-up at the **first**
    /// `followup_time` where the actual `treatment` deviates from the trial's
    /// `assigned_treatment`. The deviating row itself is **excluded**, and a
    /// later switch-back never resumes follow-up.
    PerProtocol,
}

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
    /// Causal estimand: ITT (default) or per-protocol artificial censoring.
    pub estimand: Estimand,
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
            estimand: Estimand::Itt,
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

    /// Select the causal [`Estimand`] (default [`Estimand::Itt`]).
    #[must_use]
    pub fn with_estimand(mut self, estimand: Estimand) -> Self {
        self.estimand = estimand;
        self
    }
}

/// Expand a prepared person-time [`LazyFrame`] into the sequential
/// target-trial layout.
///
/// The returned frame carries exactly the six structural columns, in order:
/// `id, trial_period, followup_time, assigned_treatment, treatment, outcome`,
/// sorted by `(id, trial_period, followup_time)`.
///
/// # Estimand
/// With [`Estimand::Itt`] (the default) follow-up runs to each patient's last
/// observed period. With [`Estimand::PerProtocol`] each trial is artificially
/// censored at the first `followup_time` where `treatment` deviates from
/// `assigned_treatment`: the deviating row is dropped and a later switch-back
/// never resumes follow-up. PP is exactly the ITT expansion with those rows
/// removed — same columns, dtypes and order, fewer rows.
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

    match options.estimand {
        // ITT: return the full expansion, byte-identical to Phase 1.
        Estimand::Itt => Ok(expanded),
        // PP: keep only rows STRICTLY BEFORE each trial's first deviation.
        // Within each `(id, trial_period)` window, ordered explicitly by
        // `followup_time`, the cumulative max of the deviation flag
        // (`treatment != assigned_treatment`) is `0` exactly on the adherent
        // prefix and flips to `1` at the first deviation — which also discards
        // every later row, so a switch-back cannot resume follow-up. The
        // baseline row never deviates by construction. The explicit `order_by`
        // makes the cumulative independent of physical row order (determinism).
        Estimand::PerProtocol => {
            const COL_CUMDEV: &str = "__tte_cumulative_deviation";
            let cumulative_deviation = col(treatment)
                .neq(col(COL_ASSIGNED_TREATMENT))
                .cast(DataType::Int32)
                .cum_max(false)
                .over_with_options(
                    Some(vec![col(id), col(COL_TRIAL_PERIOD)]),
                    Some((vec![col(COL_FOLLOWUP_TIME)], SortOptions::default())),
                    WindowMapping::default(),
                )?
                .alias(COL_CUMDEV);
            let censored = expanded
                .with_column(cumulative_deviation)
                .filter(col(COL_CUMDEV).eq(lit(0i32)))
                .select([
                    col(id),
                    col(COL_TRIAL_PERIOD),
                    col(COL_FOLLOWUP_TIME),
                    col(COL_ASSIGNED_TREATMENT),
                    col(treatment),
                    col(outcome),
                ]);
            Ok(censored)
        },
    }
}

/// Read the Parquet file at `input_path`, expand it under `options.estimand`
/// (ITT by default, or per-protocol), and write the dtype-exact result to
/// `output_path`.
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

/// Apply pre-computed inverse-probability weights to a structural expansion.
///
/// `expanded` is the six structural columns produced by [`expand`]; `factors` is
/// the per-`(id, period)` IPW factor table emitted by the Oracle, with columns
/// `id`, `period` and `weight_factor`. The per-row `weight` is the **cumulative
/// product**, within each `(id, trial_period)` ordered by `followup_time`, of the
/// factor joined on `(id, period := trial_period + followup_time)`, with the
/// baseline (`followup_time == 0`) multiplier forced to `1.0` (SPEC §4). The
/// estimand selects the *weight model* upstream (in R) but not this arithmetic —
/// join + cumulative product is identical for ITT and per-protocol.
///
/// The returned frame is the six structural columns followed by `weight`
/// (Float64), sorted by `(id, trial_period, followup_time)`. Weight *values* come
/// from R; the engine only reproduces their deterministic accumulation, so the
/// structural columns match the Oracle exactly while `weight` matches within the
/// harness's float tolerance (ADR-2).
///
/// # Errors
/// Returns [`ExpandError`] if a Polars operation fails (e.g. the window
/// evaluation or the join cannot be planned).
pub fn apply_weights(
    expanded: LazyFrame,
    factors: LazyFrame,
    options: &ExpandOptions,
) -> Result<LazyFrame> {
    // Internal column holding the per-row multiplier before it is accumulated.
    const COL_MULT: &str = "__tte_weight_multiplier";

    let id = options.id_col.as_str();
    let treatment = options.treatment_col.as_str();
    let outcome = options.outcome_col.as_str();

    // Each follow-up row's calendar period is the factor-table join key. Both
    // `trial_period` and `followup_time` are integer, so the sum is integer; cast
    // to Int32 to match the factor table's `period` dtype exactly.
    let with_period = expanded.with_column(
        (col(COL_TRIAL_PERIOD) + col(COL_FOLLOWUP_TIME))
            .cast(DataType::Int32)
            .alias(COL_PERIOD),
    );

    // Per-row multiplier: 1.0 at the trial baseline (it accrues no weight), else
    // the joined per-`(id, period)` factor. A missing factor on a follow-up row
    // would surface as a null `weight` (a loud failure), never a silent 1.0.
    let multiplier = when(col(COL_FOLLOWUP_TIME).eq(lit(0i32)))
        .then(lit(1.0))
        .otherwise(col(COL_WEIGHT_FACTOR))
        .alias(COL_MULT);

    // `weight` is the cumulative product of the multiplier within each trial,
    // ordered explicitly by `followup_time` so it is independent of physical row
    // order (determinism) — the `cum_prod` analogue of the per-protocol `cum_max`.
    let weight = col(COL_MULT)
        .cum_prod(false)
        .over_with_options(
            Some(vec![col(id), col(COL_TRIAL_PERIOD)]),
            Some((vec![col(COL_FOLLOWUP_TIME)], SortOptions::default())),
            WindowMapping::default(),
        )?
        .cast(DataType::Float64)
        .alias(COL_WEIGHT);

    let weighted = with_period
        .join(
            factors,
            [col(id), col(COL_PERIOD)],
            [col(id), col(COL_PERIOD)],
            JoinArgs::new(JoinType::Left),
        )
        .with_column(multiplier)
        .with_column(weight)
        .select([
            col(id),
            col(COL_TRIAL_PERIOD),
            col(COL_FOLLOWUP_TIME),
            col(COL_ASSIGNED_TREATMENT),
            col(treatment),
            col(outcome),
            col(COL_WEIGHT),
        ])
        // (id, trial_period, followup_time) is a unique key — total, deterministic.
        .sort_by_exprs(
            [col(id), col(COL_TRIAL_PERIOD), col(COL_FOLLOWUP_TIME)],
            SortMultipleOptions::default()
                .with_order_descending(false)
                .with_maintain_order(true),
        );

    Ok(weighted)
}

/// Expand `input_path` and attach pre-computed weights, writing the result.
///
/// Reads the structural input, expands it under `options.estimand`, joins the
/// per-`(id, period)` factors from `factors_path`, and writes the weighted frame
/// (six structural columns + `weight`) to `output_path`. This is [`expand`]
/// followed by [`apply_weights`]; see the latter for the weighting rule.
///
/// # Errors
/// Returns [`ExpandError`] if reading/writing fails, a path is not valid UTF-8,
/// `options` are invalid, or a Polars operation fails.
pub fn expand_weighted_parquet(
    input_path: impl AsRef<Path>,
    factors_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    options: &ExpandOptions,
) -> Result<()> {
    let input_str = input_path
        .as_ref()
        .to_str()
        .ok_or_else(|| ExpandError::InvalidOptions("input path is not valid UTF-8".to_owned()))?;
    let factors_str = factors_path
        .as_ref()
        .to_str()
        .ok_or_else(|| ExpandError::InvalidOptions("factors path is not valid UTF-8".to_owned()))?;
    let input = LazyFrame::scan_parquet(PlRefPath::new(input_str), ScanArgsParquet::default())?;
    let factors = LazyFrame::scan_parquet(PlRefPath::new(factors_str), ScanArgsParquet::default())?;
    let expanded = expand(input, options)?;
    let mut frame = apply_weights(expanded, factors, options)?.collect()?;
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

    use super::{Estimand, ExpandOptions, apply_weights, expand, expand_parquet};
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

    /// Read a column as `i64` (casting), null-free — for invariant checks.
    fn col_i64(df: &DataFrame, name: &str) -> Vec<i64> {
        df.column(name)
            .expect("column")
            .cast(&DataType::Int64)
            .expect("cast i64")
            .i64()
            .expect("i64")
            .into_no_null_iter()
            .collect()
    }

    /// Read a column as `f64` (casting), null-free — for invariant checks.
    fn col_f64(df: &DataFrame, name: &str) -> Vec<f64> {
        df.column(name)
            .expect("column")
            .cast(&DataType::Float64)
            .expect("cast f64")
            .f64()
            .expect("f64")
            .into_no_null_iter()
            .collect()
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

    /// Expand `fixtures/<subdir>/input_<name>.parquet` under the per-protocol
    /// estimand and assert it equals `fixtures/<subdir>/expected_<name>_pp.parquet`
    /// exactly (schema + values + order). PP keeps the same six structural
    /// columns/dtypes as ITT; censoring shows up purely as missing rows.
    fn assert_pp_matches(subdir: &str, name: &str) {
        let input = fixture(&format!("{subdir}/input_{name}.parquet"));
        let expected_path = fixture(&format!("{subdir}/expected_{name}_pp.parquet"));
        assert!(input.exists(), "missing input fixture: {}", input.display());
        assert!(
            expected_path.exists(),
            "missing expected fixture: {}",
            expected_path.display()
        );

        let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX)
            .with_estimand(Estimand::PerProtocol);
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

    // ---- Per-protocol (PP) artificial censoring: same six structural columns,
    //      censoring shows up as missing rows. PP==ITT for the control-only /
    //      single-row cases (E01/E03/E05/E07/E08/E09); divergence on E02/E04/E06
    //      and every scenario. ----
    #[test]
    fn pp_e01_single() {
        assert_pp_matches("edge", "E01_single");
    }

    #[test]
    fn pp_e02_id4_canonical_divergence() {
        assert_pp_matches("edge", "E02_id4_canonical");
    }

    #[test]
    fn pp_e03_event_at_baseline() {
        assert_pp_matches("edge", "E03_event_at_baseline");
    }

    #[test]
    fn pp_e04_reentry_divergence() {
        assert_pp_matches("edge", "E04_reentry");
    }

    #[test]
    fn pp_e05_never_treats() {
        assert_pp_matches("edge", "E05_never_treats");
    }

    // E06 is the canonical switch-back trap (treatment 1,1,0,1; assigned=1): PP
    // must censor at the first deviation (followup_time 2) and NOT resume at the
    // switch-back (followup_time 3), leaving exactly followup_time 0 and 1.
    #[test]
    fn pp_e06_switch_then_back() {
        assert_pp_matches("edge", "E06_switch_then_back");
    }

    #[test]
    fn pp_e07_last_period_only() {
        assert_pp_matches("edge", "E07_last_period_only");
    }

    #[test]
    fn pp_e08_ties() {
        assert_pp_matches("edge", "E08_ties");
    }

    #[test]
    fn pp_e09_max_fanout() {
        assert_pp_matches("edge", "E09_max_fanout");
    }

    #[test]
    fn pp_scenario_common() {
        assert_pp_matches("scenarios", "common");
    }

    #[test]
    fn pp_scenario_rare_event() {
        assert_pp_matches("scenarios", "rare_event");
    }

    #[test]
    fn pp_scenario_ultra_rare_event() {
        assert_pp_matches("scenarios", "ultra_rare_event");
    }

    #[test]
    fn pp_scenario_rare_initiation() {
        assert_pp_matches("scenarios", "rare_initiation");
    }

    #[test]
    fn pp_scenario_high_switching() {
        assert_pp_matches("scenarios", "high_switching");
    }

    #[test]
    fn pp_scenario_heavy_censoring() {
        assert_pp_matches("scenarios", "heavy_censoring");
    }

    #[test]
    fn pp_scenario_short_followup() {
        assert_pp_matches("scenarios", "short_followup");
    }

    #[test]
    fn pp_scenario_strong_confounding() {
        assert_pp_matches("scenarios", "strong_confounding");
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

    #[test]
    fn invariant_pp_monotone_censoring() {
        // SPEC §5 (PP): once a trial deviates it is censored — the retained
        // follow-up is a contiguous prefix 0..k of ADHERENT rows, with NO row at
        // or after the first deviation (so a switch-back never resumes). This is
        // derived from the input fixtures alone, independent of the PP expected
        // fixtures, so it stands as a true invariant rather than a tautology.
        // (id, trial_period) -> rows (followup_time, treatment, assigned), in the
        // engine's canonical followup_time order.
        fn grouped(df: &DataFrame) -> std::collections::BTreeMap<(i64, i64), Vec<(i64, i64, i64)>> {
            let id = col_i64(df, "id");
            let tp = col_i64(df, "trial_period");
            let fu = col_i64(df, "followup_time");
            let trt = col_i64(df, "treatment");
            let asg = col_i64(df, "assigned_treatment");
            let mut m: std::collections::BTreeMap<(i64, i64), Vec<(i64, i64, i64)>> =
                std::collections::BTreeMap::new();
            for ((((id, tp), fu), trt), asg) in id.into_iter().zip(tp).zip(fu).zip(trt).zip(asg) {
                m.entry((id, tp)).or_default().push((fu, trt, asg));
            }
            m
        }

        for name in ["E02_id4_canonical", "E04_reentry", "E06_switch_then_back"] {
            let input = fixture(&format!("edge/input_{name}.parquet"));
            let scan = || {
                LazyFrame::scan_parquet(
                    PlRefPath::new(input.to_str().expect("utf8")),
                    ScanArgsParquet::default(),
                )
                .expect("scan")
            };
            let itt_opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX);
            let pp_opts = itt_opts.clone().with_estimand(Estimand::PerProtocol);
            let itt = expand(scan(), &itt_opts)
                .expect("itt")
                .collect()
                .expect("collect itt");
            let pp = expand(scan(), &pp_opts)
                .expect("pp")
                .collect()
                .expect("collect pp");

            let itt_g = grouped(&itt);
            let pp_g = grouped(&pp);

            for (key, itt_rows) in &itt_g {
                // Expected PP keep-count = length of the leading adherent run.
                let keep = itt_rows
                    .iter()
                    .position(|&(_, trt, asg)| trt != asg)
                    .unwrap_or(itt_rows.len());
                let pp_rows = pp_g.get(key).cloned().unwrap_or_default();
                assert_eq!(
                    pp_rows.len(),
                    keep,
                    "[{name}] trial {key:?}: PP kept {} rows, expected the {keep}-row leading adherent prefix",
                    pp_rows.len()
                );
                for (idx, (fu, trt, asg)) in pp_rows.into_iter().enumerate() {
                    let expected_fu = i64::try_from(idx).expect("index fits i64");
                    assert_eq!(
                        fu, expected_fu,
                        "[{name}] trial {key:?}: PP follow-up is not a contiguous 0.. prefix"
                    );
                    assert_eq!(
                        trt, asg,
                        "[{name}] trial {key:?}: PP retained a non-adherent row at followup_time {fu}"
                    );
                }
            }

            // PP trials are a subset of ITT trials (censoring only removes rows).
            for key in pp_g.keys() {
                assert!(
                    itt_g.contains_key(key),
                    "[{name}] PP trial {key:?} absent from the ITT expansion"
                );
            }
        }

        // Concrete switch-back trap: E06 (treatment 1,1,0,1; assigned=1) keeps
        // exactly followup_time {0,1}; the deviation (fu=2) and switch-back (fu=3)
        // are both gone.
        let input = fixture("edge/input_E06_switch_then_back.parquet");
        let pp = expand(
            LazyFrame::scan_parquet(
                PlRefPath::new(input.to_str().expect("utf8")),
                ScanArgsParquet::default(),
            )
            .expect("scan"),
            &ExpandOptions::new("id", "period", "treatment", 0, i32::MAX)
                .with_estimand(Estimand::PerProtocol),
        )
        .expect("pp")
        .collect()
        .expect("collect");
        let fu = col_i64(&pp, "followup_time");
        assert_eq!(
            fu,
            vec![0, 1],
            "E06 PP must keep exactly followup_time 0 and 1 (switch-back at fu=3 excluded)"
        );
    }

    #[test]
    fn invariant_weight_cumulative_product() {
        // SPEC §5 (Weighted): once weights are applied, (a) every baseline row
        // has weight == 1.0, (b) weight > 0 everywhere, (c) the per-(id, period)
        // factor recovered as weight[t]/weight[t-1] is invariant across the
        // overlapping trials that share an (id, period) — i.e. the engine applied
        // a per-(id, period) multiplier, not a per-trial one — and (d) the six
        // structural columns are byte-identical to the unweighted expansion (the
        // weighting only appends `weight`). Checked on high_switching (many
        // overlapping trials) and data_censored (combined switch + censor).
        use std::collections::BTreeMap;

        for (subdir, name) in [
            ("scenarios", "high_switching"),
            ("weights", "data_censored"),
        ] {
            let input = fixture(&format!("{subdir}/input_{name}.parquet"));
            let factors_path = fixture(&format!("weights/input_{name}_pp_weights.parquet"));
            let opts = ExpandOptions::new("id", "period", "treatment", 0, i32::MAX)
                .with_estimand(Estimand::PerProtocol);
            let scan = |p: &Path| {
                LazyFrame::scan_parquet(
                    PlRefPath::new(p.to_str().expect("utf8")),
                    ScanArgsParquet::default(),
                )
                .expect("scan")
            };
            let structural = expand(scan(&input), &opts)
                .expect("expand")
                .collect()
                .expect("collect structural");
            let weighted = apply_weights(
                expand(scan(&input), &opts).expect("expand"),
                scan(&factors_path),
                &opts,
            )
            .expect("apply_weights")
            .collect()
            .expect("collect weighted");

            // (d) structural columns unchanged; `weight` appended as the 7th.
            assert_eq!(
                weighted.width(),
                7,
                "[{name}] weighted frame must be the 6 structural columns + weight"
            );
            for c in [
                "id",
                "trial_period",
                "followup_time",
                "assigned_treatment",
                "treatment",
                "outcome",
            ] {
                assert!(
                    weighted
                        .column(c)
                        .expect("weighted column")
                        .equals(structural.column(c).expect("structural column")),
                    "[{name}] structural column '{c}' changed under weighting"
                );
            }

            let id = col_i64(&weighted, "id");
            let tp = col_i64(&weighted, "trial_period");
            let fu = col_i64(&weighted, "followup_time");
            let w = col_f64(&weighted, "weight");

            // (a) baseline weight == 1.0; (b) weight > 0 everywhere.
            for ((&fu_i, &w_i), _) in fu.iter().zip(&w).zip(&id) {
                assert!(w_i > 0.0, "[{name}] non-positive weight {w_i}");
                if fu_i == 0 {
                    assert!(
                        (w_i - 1.0).abs() < 1e-12,
                        "[{name}] baseline weight {w_i} != 1.0"
                    );
                }
            }

            // (c) per-(id, period) factor invariance across overlapping trials.
            // Rows are sorted by (id, trial_period, followup_time), so the prior
            // row of the same trial is the previous follow-up — its weight is the
            // denominator of the incremental factor.
            let mut last: BTreeMap<(i64, i64), f64> = BTreeMap::new();
            let mut factor_of: BTreeMap<(i64, i64), f64> = BTreeMap::new();
            let mut checked = 0usize;
            for (((&id_i, &tp_i), &fu_i), &w_i) in id.iter().zip(&tp).zip(&fu).zip(&w) {
                if fu_i > 0 {
                    let prev = last
                        .get(&(id_i, tp_i))
                        .copied()
                        .expect("previous follow-up row in trial");
                    let factor = w_i / prev;
                    let period = tp_i + fu_i;
                    if let Some(&seen) = factor_of.get(&(id_i, period)) {
                        assert!(
                            (factor - seen).abs() <= 1e-9 * seen.abs().max(1.0),
                            "[{name}] factor at (id {id_i}, period {period}) varies across trials: {factor} vs {seen}"
                        );
                        checked += 1;
                    } else {
                        factor_of.insert((id_i, period), factor);
                    }
                }
                last.insert((id_i, tp_i), w_i);
            }
            assert!(
                checked > 0,
                "[{name}] expected overlapping trials sharing an (id, period) cell"
            );
        }
    }
}
