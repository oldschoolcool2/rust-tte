//! Phase 6 — inverse-probability **weight fitting** in Rust.
//!
//! Phase 3 ([`crate::apply_weights`]) *consumes* a per-`(id, period)` factor
//! table; this module *produces* it, reproducing the legacy
//! `TrialEmulation::data_preparation(use_censor_weights = …)` weight path:
//!
//! 1. **Design preparation** (a faithful port of the package's `data_manipulation`
//!    + the compiled `censor_func` state machine): drop pre-eligibility and
//!    post-event person-periods, lag treatment to `am_1`, derive the `switch`
//!    flag, and — for the per-protocol estimand — run the artificial-censoring
//!    state machine that selects the rows at risk of switching. This is pure
//!    deterministic data transformation (Rust's job), no statistics.
//! 2. **Model fitting**: bind [`smartcore`]'s unregularised binomial-logit
//!    solver (`alpha = 0`; deterministic L-BFGS from a zero start) to fit the
//!    switching weight models (numerator/denominator × previous-treatment
//!    stratum) and/or the IPCW censoring models. The crate is **not** allowed to
//!    hand-roll IRLS; robust/sandwich variance and the MSM stay in R.
//! 3. **Combination**: per person-period, form the stabilised switch weight
//!    `p_n / p_d` (or `(1-p_n)/(1-p_d)`) and the IPCW factor `pC_n / pC_d`, and
//!    multiply them — exactly the per-`(id, period)` `wt` the Oracle emits and
//!    that [`crate::apply_weights`] accumulates into the cumulative `weight`.
//!
//! ## Where exactness ends (ADR-2, staged tolerance)
//! The structural columns stay **bit-exact** and the applied-weight product stays
//! within **~1e-12**, but these *fitted* factors are reproduced only within a
//! documented **~1e-6** tolerance: `smartcore`'s L-BFGS converges *to* the same
//! maximum-likelihood estimate as R's IRLS `glm` (observed coefficient agreement
//! ~1.6e-8 on the fixtures), but not bit-for-bit. A bit-exact assertion would be
//! wrong here — unlike the deterministic expansion/censoring columns.

use polars::prelude::*;
// NB: smartcore's `Array`/`Array2` traits implement `.get(i) -> &T` for `Vec`/
// `[T]`, which would shadow std slice `.get() -> Option<&T>` across this module.
// They are therefore imported *locally* in `logit_params` (the only place that
// indexes a smartcore matrix), so every other `.get()` here stays std slice.
use smartcore::linalg::basic::matrix::DenseMatrix;
use smartcore::linear::logistic_regression::{LogisticRegression, LogisticRegressionParameters};

use crate::{
    COL_PERIOD, COL_WEIGHT_FACTOR, Estimand, ExpandError, ExpandOptions, Result, apply_weights,
    expand,
};

/// How the IPCW censoring numerator/denominator models are pooled across the
/// previous-treatment (`am_1`) strata.
///
/// Mirrors `TrialEmulation`'s `pool_cense`: the numerator is pooled for
/// [`PoolCensor::Numerator`] and [`PoolCensor::Both`]; the denominator is pooled
/// only for [`PoolCensor::Both`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PoolCensor {
    /// Fit numerator and denominator separately within each `am_1` stratum
    /// (the per-protocol default).
    None,
    /// Pool the numerator across strata; keep the denominator per-stratum
    /// (the intention-to-treat default).
    Numerator,
    /// Pool both numerator and denominator across strata.
    Both,
}

impl PoolCensor {
    /// Whether the numerator model is pooled across `am_1` strata.
    const fn pool_numerator(self) -> bool {
        matches!(self, Self::Numerator | Self::Both)
    }

    /// Whether the denominator model is pooled across `am_1` strata.
    const fn pool_denominator(self) -> bool {
        matches!(self, Self::Both)
    }
}

/// Per-protocol **switching**-weight model specification.
///
/// Both models regress the observed `treatment` on an intercept plus the named
/// covariate columns (a logit link), fitted separately within the previously
/// untreated (`am_1 == 0`) and previously treated (`am_1 == 1`) strata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct SwitchWeightSpec {
    /// Covariate columns for the (stabilising) numerator model.
    pub numerator_covariates: Vec<String>,
    /// Covariate columns for the denominator model.
    pub denominator_covariates: Vec<String>,
}

impl SwitchWeightSpec {
    /// Build a switching-weight spec from numerator and denominator covariate
    /// column names.
    pub fn new<S: Into<String>>(
        numerator_covariates: impl IntoIterator<Item = S>,
        denominator_covariates: impl IntoIterator<Item = S>,
    ) -> Self {
        Self {
            numerator_covariates: numerator_covariates.into_iter().map(Into::into).collect(),
            denominator_covariates: denominator_covariates.into_iter().map(Into::into).collect(),
        }
    }
}

/// Inverse-probability-of-**censoring** (IPCW) model specification.
///
/// Both models regress *remaining uncensored* (`1 - censor_col`) on an intercept
/// plus the named covariates (logit link). [`Self::pool`] selects how the two
/// models are shared across the `am_1` strata.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CensorWeightSpec {
    /// Name of the `{0,1}` censoring-indicator column; the response is
    /// `1 - censor_col` (the probability of remaining uncensored).
    pub censor_col: String,
    /// Covariate columns for the numerator model.
    pub numerator_covariates: Vec<String>,
    /// Covariate columns for the denominator model.
    pub denominator_covariates: Vec<String>,
    /// Pooling of the numerator/denominator models across `am_1` strata.
    pub pool: PoolCensor,
}

impl CensorWeightSpec {
    /// Build an IPCW spec from the censoring column, the numerator/denominator
    /// covariates, and the pooling scheme.
    pub fn new<S: Into<String>>(
        censor_col: impl Into<String>,
        numerator_covariates: impl IntoIterator<Item = S>,
        denominator_covariates: impl IntoIterator<Item = S>,
        pool: PoolCensor,
    ) -> Self {
        Self {
            censor_col: censor_col.into(),
            numerator_covariates: numerator_covariates.into_iter().map(Into::into).collect(),
            denominator_covariates: denominator_covariates.into_iter().map(Into::into).collect(),
            pool,
        }
    }
}

/// Full specification for fitting the per-`(id, period)` IPW factor.
///
/// Either component may be present: switching weights (per-protocol) and/or IPCW
/// censoring weights, multiplied together where both apply. The estimand on
/// [`ExpandOptions`] selects whether the artificial-censoring state machine runs
/// (per-protocol) and provides the conventional `pool_cense` default.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct WeightSpec {
    /// Per-protocol switching-weight models (omit for intention-to-treat).
    pub switch: Option<SwitchWeightSpec>,
    /// Inverse-probability-of-censoring models (omit when there is no censoring).
    pub censor: Option<CensorWeightSpec>,
}

impl WeightSpec {
    /// Per-protocol switching weights only (no IPCW).
    #[must_use]
    pub fn switching(switch: SwitchWeightSpec) -> Self {
        Self {
            switch: Some(switch),
            censor: None,
        }
    }

    /// Inverse-probability-of-censoring weights only (no switching).
    #[must_use]
    pub fn ipcw(censor: CensorWeightSpec) -> Self {
        Self {
            switch: None,
            censor: Some(censor),
        }
    }

    /// Attach IPCW censoring weights to a switching spec (per-protocol combined).
    #[must_use]
    pub fn with_censor(mut self, censor: CensorWeightSpec) -> Self {
        self.censor = Some(censor);
        self
    }
}

/// A fitted unregularised binomial-logit model: intercept plus one coefficient
/// per covariate (in the covariate order it was fitted with).
struct Logit {
    intercept: f64,
    coefficients: Vec<f64>,
}

impl Logit {
    /// Predicted probability `P(y = 1 | x)` for the covariate values at `row`.
    fn predict(&self, covariates: &[&[f64]], row: usize) -> Result<f64> {
        let mut eta = self.intercept;
        for (column, beta) in covariates.iter().zip(self.coefficients.iter()) {
            let x = column
                .get(row)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("prediction row out of range".to_owned()))?;
            eta += beta * x;
        }
        Ok(1.0 / (1.0 + (-eta).exp()))
    }
}

/// Fit `y ~ 1 + covariates` (binomial logit, unregularised) over `rows`.
///
/// Binds `smartcore`'s deterministic L-BFGS solver (`alpha = 0`, zero start). An
/// empty covariate list degenerates to an intercept-only fit computed in closed
/// form. Returns coefficients in the original covariate coordinates (matching R
/// `glm`), intercept first.
fn fit_logit(rows: &[usize], covariates: &[&[f64]], response: &[i32]) -> Result<Logit> {
    if rows.is_empty() {
        return Err(ExpandError::WeightFit(
            "empty design matrix (no rows to fit)".to_owned(),
        ));
    }
    if covariates.is_empty() {
        // Intercept-only: the MLE is logit(mean(y)). Accumulate in f64 (lossless
        // for 0/1 counts) to avoid integer→float casts.
        let mut n = 0.0_f64;
        let mut ones = 0.0_f64;
        for &i in rows {
            let y = response
                .get(i)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("response row out of range".to_owned()))?;
            n += 1.0;
            ones += f64::from(y);
        }
        let mean = ones / n;
        if !(mean > 0.0 && mean < 1.0) {
            return Err(ExpandError::WeightFit(
                "intercept-only model is perfectly separated".to_owned(),
            ));
        }
        return Ok(Logit {
            intercept: (mean / (1.0 - mean)).ln(),
            coefficients: Vec::new(),
        });
    }

    let design: Vec<Vec<f64>> = rows
        .iter()
        .map(|&i| {
            covariates
                .iter()
                .map(|column| {
                    column.get(i).copied().ok_or_else(|| {
                        ExpandError::WeightFit("covariate row out of range".to_owned())
                    })
                })
                .collect::<Result<Vec<f64>>>()
        })
        .collect::<Result<Vec<Vec<f64>>>>()?;
    let labels: Vec<i32> = rows
        .iter()
        .map(|&i| {
            response
                .get(i)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("response row out of range".to_owned()))
        })
        .collect::<Result<Vec<i32>>>()?;

    let design_ref: Vec<&[f64]> = design.iter().map(Vec::as_slice).collect();
    let matrix = DenseMatrix::from_2d_array(&design_ref);
    let params = LogisticRegressionParameters::default().with_alpha(0.0);
    let model = LogisticRegression::fit(&matrix, &labels, params)
        .map_err(|e| ExpandError::WeightFit(format!("logistic solver failed: {e}")))?;
    Ok(logit_params(&model, covariates.len()))
}

/// Extract intercept + `p` coefficients from a fitted `smartcore` model.
///
/// `coefficients()` is a 1×`p` row vector and `intercept()` a 1×1, both in the
/// original (un-standardised) coordinate system that matches R `glm`. Class
/// labels are sorted `{0, 1}`, so the fitted probability is `P(y = 1)`. The
/// `Array` trait is imported *here only* (it shadows std slice `.get`).
fn logit_params(
    model: &LogisticRegression<f64, i32, DenseMatrix<f64>, Vec<i32>>,
    p: usize,
) -> Logit {
    use smartcore::linalg::basic::arrays::Array;
    let intercept = *model.intercept().get((0, 0));
    let coefficients: Vec<f64> = (0..p).map(|j| *model.coefficients().get((0, j))).collect();
    Logit {
        intercept,
        coefficients,
    }
}

/// The prepared per-person-period design frame (a faithful `data_manipulation`
/// output), as column-major typed vectors aligned by row.
struct Design {
    id: Vec<i64>,
    period: Vec<i32>,
    treatment: Vec<i32>,
    /// Previous-period treatment (`am_1`); `0` on each subject's first row.
    am_1: Vec<i32>,
    /// Optional remaining-uncensored response (`1 - censor`), present iff IPCW.
    uncensored: Option<Vec<i32>>,
    /// Named covariate columns (looked up by name; never iterated for output).
    covariates: Vec<(String, Vec<f64>)>,
}

impl Design {
    fn len(&self) -> usize {
        self.id.len()
    }

    /// Row indices in a previous-treatment stratum (`am_1 == am`).
    fn stratum(&self, am: i32) -> Vec<usize> {
        self.am_1
            .iter()
            .enumerate()
            .filter_map(|(i, &a)| (a == am).then_some(i))
            .collect()
    }

    /// All row indices.
    fn all_rows(&self) -> Vec<usize> {
        (0..self.len()).collect()
    }

    /// Resolve named covariate columns to row-indexable slices.
    fn columns<'a>(&'a self, names: &[String]) -> Result<Vec<&'a [f64]>> {
        names
            .iter()
            .map(|name| {
                self.covariates
                    .iter()
                    .find_map(|(n, v)| (n == name).then_some(v.as_slice()))
                    .ok_or_else(|| {
                        ExpandError::WeightFit(format!("weight covariate '{name}' not found"))
                    })
            })
            .collect()
    }
}

/// Read a column, cast to `Int32`, into a null-free `Vec<i32>`.
fn column_i32(frame: &DataFrame, name: &str) -> Result<Vec<i32>> {
    let casted = frame.column(name)?.cast(&DataType::Int32)?;
    casted
        .i32()?
        .iter()
        .map(|o| o.ok_or_else(|| ExpandError::WeightFit(format!("null value in column '{name}'"))))
        .collect()
}

/// Read a column, cast to `Int64`, into a null-free `Vec<i64>`.
fn column_i64(frame: &DataFrame, name: &str) -> Result<Vec<i64>> {
    let casted = frame.column(name)?.cast(&DataType::Int64)?;
    casted
        .i64()?
        .iter()
        .map(|o| o.ok_or_else(|| ExpandError::WeightFit(format!("null value in column '{name}'"))))
        .collect()
}

/// Read a column, cast to `Float64`, into a null-free `Vec<f64>`.
fn column_f64(frame: &DataFrame, name: &str) -> Result<Vec<f64>> {
    let casted = frame.column(name)?.cast(&DataType::Float64)?;
    casted
        .f64()?
        .iter()
        .map(|o| o.ok_or_else(|| ExpandError::WeightFit(format!("null value in column '{name}'"))))
        .collect()
}

/// The artificial-censoring state machine (`censor_func`) for the per-protocol
/// path: a deterministic per-subject sequential scan returning the keep mask.
///
/// A faithful port of `TrialEmulation`'s compiled `censor_func`: a row is kept
/// iff the subject is, at that period, eligible to contribute to a switching
/// model (`eligible0_sw == 1 || eligible1_sw == 1`); otherwise it is dropped.
/// State resets at each subject's first row and whenever a regime stops. Inputs
/// must be ordered by `(id, period)` with `first` marking each subject's first
/// retained row.
fn censor_keep_mask(
    first: &[bool],
    eligible: &[i32],
    treatment: &[i32],
    switch: &[i32],
) -> Result<Vec<bool>> {
    let n = first.len();
    let mut keep = vec![false; n];
    // Persistent regime state; the `eligible*_sw` flags are recomputed every row
    // (as in the C++), so they live per-iteration, not across rows.
    let (mut started0, mut started1, mut stop0, mut stop1) = (0_i32, 0, 0, 0);

    for i in 0..n {
        let at = |s: &[i32]| -> Result<i32> {
            s.get(i)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("censor scan row out of range".to_owned()))
        };
        let is_first = *first
            .get(i)
            .ok_or_else(|| ExpandError::WeightFit("censor scan row out of range".to_owned()))?;
        let (elig_i, treat_i, switch_i) = (at(eligible)?, at(treatment)?, at(switch)?);

        if is_first || stop0 == 1 || stop1 == 1 {
            (started0, started1, stop0, stop1) = (0, 0, 0, 0);
        }
        if started0 == 0 && started1 == 0 && elig_i == 1 {
            if treat_i == 0 {
                started0 = 1;
            } else if treat_i == 1 {
                started1 = 1;
            }
        }
        let (mut elig0, mut elig1) = if started0 == 1 && stop0 == 0 {
            (1, 0)
        } else if started1 == 1 && stop1 == 0 {
            (0, 1)
        } else {
            (0, 0)
        };
        if switch_i == 1 {
            if elig_i == 1 {
                if treat_i == 1 {
                    (started1, stop1, started0, stop0, elig1) = (1, 0, 0, 0, 1);
                } else if treat_i == 0 {
                    (started0, stop0, started1, stop1, elig0) = (1, 0, 0, 0, 1);
                }
            } else {
                stop0 = started0;
                stop1 = started1;
            }
        }
        if let Some(slot) = keep.get_mut(i) {
            *slot = elig0 != 0 || elig1 != 0;
        }
    }
    Ok(keep)
}

/// Build the [`Design`] frame from the raw cohort: sort, drop pre-eligibility and
/// post-event person-periods, derive `am_1`/`switch`, and — for the per-protocol
/// estimand — apply the artificial-censoring keep mask.
fn prepare_design(cohort: LazyFrame, options: &ExpandOptions, spec: &WeightSpec) -> Result<Design> {
    let raw = collect_columns(cohort, options, spec)?;
    let kept = derive_retained(&raw)?;

    // Per-protocol: run the artificial-censoring state machine over the retained
    // rows and keep only the at-risk-of-switching person-periods.
    let survivors: Vec<usize> = if matches!(options.estimand, Estimand::PerProtocol) {
        let kept_eligible = gather(
            &(0..kept.kept.len()).collect::<Vec<_>>(),
            &kept.kept,
            &raw.eligible,
        )?;
        let kept_treatment = gather(
            &(0..kept.kept.len()).collect::<Vec<_>>(),
            &kept.kept,
            &raw.treatment,
        )?;
        let mask = censor_keep_mask(&kept.first, &kept_eligible, &kept_treatment, &kept.switch)?;
        (0..kept.kept.len())
            .filter(|&k| mask.get(k).copied().unwrap_or(false))
            .collect()
    } else {
        (0..kept.kept.len()).collect()
    };

    // Project every column onto the surviving rows (`gather` follows `survivor →
    // kept[survivor] → raw row`; `gather_direct` reads kept-aligned vectors).
    let uncensored = raw
        .censor
        .as_ref()
        .map(|c| {
            let raw_censor = gather(&survivors, &kept.kept, c)?;
            Ok::<_, ExpandError>(raw_censor.into_iter().map(|v| 1 - v).collect())
        })
        .transpose()?;
    let covariates = raw
        .covariates
        .iter()
        .map(|(name, values)| Ok((name.clone(), gather(&survivors, &kept.kept, values)?)))
        .collect::<Result<_>>()?;

    Ok(Design {
        id: gather(&survivors, &kept.kept, &raw.id)?,
        period: gather(&survivors, &kept.kept, &raw.period)?,
        treatment: gather(&survivors, &kept.kept, &raw.treatment)?,
        am_1: gather_direct(&survivors, &kept.am_1)?,
        uncensored,
        covariates,
    })
}

/// Raw per-person-period columns extracted from the (sorted) cohort.
struct RawColumns {
    id: Vec<i64>,
    period: Vec<i32>,
    treatment: Vec<i32>,
    eligible: Vec<i32>,
    outcome: Vec<i32>,
    censor: Option<Vec<i32>>,
    covariates: Vec<(String, Vec<f64>)>,
}

/// The `data_manipulation` retention: per subject the rows in
/// `[first eligible period, first event period]`, with `first`, `am_1`, `switch`
/// derived over them. `kept[k]` is the raw-frame row of the k-th retained row.
struct Retained {
    kept: Vec<usize>,
    first: Vec<bool>,
    am_1: Vec<i32>,
    switch: Vec<i32>,
}

/// Sort the cohort by `(id, period)` and extract every column the weight models
/// reference (binary columns as `Int32`, covariates as `Float64`), null-free.
fn collect_columns(
    cohort: LazyFrame,
    options: &ExpandOptions,
    spec: &WeightSpec,
) -> Result<RawColumns> {
    let (id, period) = (options.id_col.as_str(), options.period_col.as_str());
    let (treatment, eligible, outcome) = (
        options.treatment_col.as_str(),
        options.eligible_col.as_str(),
        options.outcome_col.as_str(),
    );

    // Every covariate referenced by any model, de-duplicated, in first-seen order.
    let mut wanted: Vec<String> = Vec::new();
    let mut push = |names: &[String]| {
        for n in names {
            if !wanted.iter().any(|w| w == n) {
                wanted.push(n.clone());
            }
        }
    };
    if let Some(sw) = spec.switch.as_ref() {
        push(&sw.numerator_covariates);
        push(&sw.denominator_covariates);
    }
    if let Some(ce) = spec.censor.as_ref() {
        push(&ce.numerator_covariates);
        push(&ce.denominator_covariates);
    }

    let mut selection = vec![
        col(id),
        col(period).cast(DataType::Int32).alias(period),
        col(treatment).cast(DataType::Int32).alias(treatment),
        col(eligible).cast(DataType::Int32).alias(eligible),
        col(outcome).cast(DataType::Int32).alias(outcome),
    ];
    if let Some(ce) = spec.censor.as_ref() {
        selection.push(
            col(&ce.censor_col)
                .cast(DataType::Int32)
                .alias(&ce.censor_col),
        );
    }
    for name in &wanted {
        selection.push(col(name).cast(DataType::Float64).alias(name));
    }

    let frame = cohort
        .sort_by_exprs(
            [col(id), col(period)],
            SortMultipleOptions::default()
                .with_order_descending(false)
                .with_maintain_order(true),
        )
        .select(selection)
        .collect()?;

    Ok(RawColumns {
        id: column_i64(&frame, id)?,
        period: column_i32(&frame, period)?,
        treatment: column_i32(&frame, treatment)?,
        eligible: column_i32(&frame, eligible)?,
        outcome: column_i32(&frame, outcome)?,
        censor: spec
            .censor
            .as_ref()
            .map(|ce| column_i32(&frame, &ce.censor_col))
            .transpose()?,
        covariates: wanted
            .iter()
            .map(|name| Ok((name.clone(), column_f64(&frame, name)?)))
            .collect::<Result<Vec<_>>>()?,
    })
}

/// Apply `data_manipulation`'s row retention and derive `first`/`am_1`/`switch`.
fn derive_retained(raw: &RawColumns) -> Result<Retained> {
    let n = raw.id.len();
    let mut out = Retained {
        kept: Vec::with_capacity(n),
        first: Vec::with_capacity(n),
        am_1: Vec::with_capacity(n),
        switch: Vec::with_capacity(n),
    };

    let mut start = 0usize;
    while start < n {
        let subject = raw.id.get(start).copied().unwrap_or_default();
        let mut end = start;
        while end < n && raw.id.get(end).copied() == Some(subject) {
            end += 1;
        }
        // First eligible and first event period for this subject.
        let (mut first_elig, mut first_event): (Option<i32>, Option<i32>) = (None, None);
        for i in start..end {
            let (p, e, o) = (
                at_i32(&raw.period, i)?,
                at_i32(&raw.eligible, i)?,
                at_i32(&raw.outcome, i)?,
            );
            if e == 1 && first_elig.is_none_or(|m| p < m) {
                first_elig = Some(p);
            }
            if o == 1 && first_event.is_none_or(|m| p < m) {
                first_event = Some(p);
            }
        }
        if let Some(elig_at) = first_elig {
            let event_at = first_event.unwrap_or(i32::MAX);
            let mut prev_treat = 0_i32;
            let mut subject_first = true;
            for i in start..end {
                let p = at_i32(&raw.period, i)?;
                if p < elig_at || p > event_at {
                    continue;
                }
                let treat = at_i32(&raw.treatment, i)?;
                let lag = if subject_first { 0 } else { prev_treat };
                out.kept.push(i);
                out.first.push(subject_first);
                out.am_1.push(lag);
                out.switch.push(i32::from(!subject_first && lag != treat));
                prev_treat = treat;
                subject_first = false;
            }
        }
        start = end;
    }
    Ok(out)
}

/// Project `source` (a raw-frame column) onto the surviving rows, following
/// `survivor → kept[survivor] → source row`.
fn gather<T: Copy>(survivors: &[usize], kept: &[usize], source: &[T]) -> Result<Vec<T>> {
    survivors
        .iter()
        .map(|&k| {
            let row = *kept
                .get(k)
                .ok_or_else(|| ExpandError::WeightFit("survivor index invalid".to_owned()))?;
            source
                .get(row)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("projected row out of range".to_owned()))
        })
        .collect()
}

/// Select `source` (a kept-aligned vector) at the surviving positions directly.
fn gather_direct<T: Copy>(survivors: &[usize], source: &[T]) -> Result<Vec<T>> {
    survivors
        .iter()
        .map(|&k| {
            source
                .get(k)
                .copied()
                .ok_or_else(|| ExpandError::WeightFit("survivor index out of range".to_owned()))
        })
        .collect()
}

/// Read element `i` of an `i32` slice with a typed error (no panicking index).
fn at_i32(source: &[i32], i: usize) -> Result<i32> {
    source
        .get(i)
        .copied()
        .ok_or_else(|| ExpandError::WeightFit("row index out of range".to_owned()))
}

/// Compute the per-row stabilised switching weight `wt` (1.0 where it does not
/// apply), reproducing `TrialEmulation::weight_func`'s switch branch.
fn switch_weights(design: &Design, spec: &SwitchWeightSpec) -> Result<Vec<f64>> {
    let num_cols = design.columns(&spec.numerator_covariates)?;
    let den_cols = design.columns(&spec.denominator_covariates)?;
    let rows0 = design.stratum(0);
    let rows1 = design.stratum(1);

    let n0 = fit_logit(&rows0, &num_cols, &design.treatment)?;
    let d0 = fit_logit(&rows0, &den_cols, &design.treatment)?;
    let n1 = fit_logit(&rows1, &num_cols, &design.treatment)?;
    let d1 = fit_logit(&rows1, &den_cols, &design.treatment)?;

    (0..design.len())
        .map(|i| {
            let am = at_i32(&design.am_1, i)?;
            let treat = at_i32(&design.treatment, i)?;
            let (num_model, den_model) = if am == 0 { (&n0, &d0) } else { (&n1, &d1) };
            let p_num = num_model.predict(&num_cols, i)?;
            let p_den = den_model.predict(&den_cols, i)?;
            Ok(if treat == 0 {
                (1.0 - p_num) / (1.0 - p_den)
            } else {
                p_num / p_den
            })
        })
        .collect()
}

/// Compute the per-row IPCW factor `wtC = pC_n / pC_d` (1.0 where it does not
/// apply), reproducing `TrialEmulation::weight_func`'s censor branch.
fn censor_weights(design: &Design, spec: &CensorWeightSpec) -> Result<Vec<f64>> {
    let uncensored = design.uncensored.as_ref().ok_or_else(|| {
        ExpandError::WeightFit("IPCW requested but censor response missing".to_owned())
    })?;
    let num_cols = design.columns(&spec.numerator_covariates)?;
    let den_cols = design.columns(&spec.denominator_covariates)?;
    let rows0 = design.stratum(0);
    let rows1 = design.stratum(1);
    let all = design.all_rows();

    // Numerator: pooled across strata, or one model per `am_1` stratum.
    let numerator = if spec.pool.pool_numerator() {
        Stratified::Pooled(fit_logit(&all, &num_cols, uncensored)?)
    } else {
        Stratified::PerStratum {
            am0: fit_logit(&rows0, &num_cols, uncensored)?,
            am1: fit_logit(&rows1, &num_cols, uncensored)?,
        }
    };
    // Denominator: pooled (only when `pool == Both`), or one model per stratum.
    let denominator = if spec.pool.pool_denominator() {
        Stratified::Pooled(fit_logit(&all, &den_cols, uncensored)?)
    } else {
        Stratified::PerStratum {
            am0: fit_logit(&rows0, &den_cols, uncensored)?,
            am1: fit_logit(&rows1, &den_cols, uncensored)?,
        }
    };

    (0..design.len())
        .map(|i| {
            let am = at_i32(&design.am_1, i)?;
            let p_num = numerator.predict(am, &num_cols, i)?;
            let p_den = denominator.predict(am, &den_cols, i)?;
            Ok(p_num / p_den)
        })
        .collect()
}

/// A weight model that is either pooled across strata or fitted per `am_1`
/// stratum, selecting the right fit at prediction time.
enum Stratified {
    Pooled(Logit),
    PerStratum { am0: Logit, am1: Logit },
}

impl Stratified {
    fn predict(&self, am: i32, covariates: &[&[f64]], row: usize) -> Result<f64> {
        match self {
            Self::Pooled(model) => model.predict(covariates, row),
            Self::PerStratum { am0, am1 } => {
                if am == 0 { am0 } else { am1 }.predict(covariates, row)
            },
        }
    }
}

/// Fit the inverse-probability **weight factor** for a cohort.
///
/// Reproduces the legacy `TrialEmulation` weight path in Rust: prepare the design
/// frame (port of `data_manipulation` + the `censor_func` state machine), fit the
/// switching and/or IPCW logistic models (bound `smartcore` solver), and form the
/// per-`(id, period)` factor `wt = wt_switch · wtC`. The returned frame has
/// columns `id` (matching the input `id` dtype), `period` (`Int32`) and
/// `weight_factor` (`Float64`) — exactly the table [`apply_weights`] consumes.
///
/// `options.estimand` selects the design: [`Estimand::PerProtocol`] runs the
/// artificial-censoring state machine and (with `spec.switch`) the switching
/// models; [`Estimand::Itt`] skips both. The fit matches R `glm`/`parglm` within
/// the staged ~1e-6 tolerance (ADR-2), not bit-for-bit.
///
/// # Errors
/// Returns [`ExpandError::WeightFit`] if a referenced column is absent or null, a
/// design matrix is empty/separated, or the solver fails; [`ExpandError::Polars`]
/// if a Polars operation fails.
pub fn fit_weights(
    cohort: LazyFrame,
    options: &ExpandOptions,
    spec: &WeightSpec,
) -> Result<LazyFrame> {
    // Preserve the input `id` dtype so the factor table joins cleanly later.
    let id_dtype = {
        let mut probe = cohort.clone();
        probe
            .collect_schema()?
            .get(options.id_col.as_str())
            .cloned()
            .ok_or_else(|| {
                ExpandError::WeightFit(format!("input is missing id column '{}'", options.id_col))
            })?
    };

    let design = prepare_design(cohort, options, spec)?;
    let n = design.len();

    let switch = match spec.switch.as_ref() {
        Some(sw) => switch_weights(&design, sw)?,
        None => vec![1.0; n],
    };
    let censor = match spec.censor.as_ref() {
        Some(ce) => censor_weights(&design, ce)?,
        None => vec![1.0; n],
    };

    let factor: Vec<f64> = switch
        .iter()
        .zip(censor.iter())
        .map(|(s, c)| s * c)
        .collect();

    let height = design.id.len();
    let id_series = Series::new("id".into(), &design.id).cast(&id_dtype)?;
    let period_series = Series::new(COL_PERIOD.into(), &design.period);
    let factor_series = Series::new(COL_WEIGHT_FACTOR.into(), &factor);
    let frame = DataFrame::new(
        height,
        vec![
            id_series.into_column(),
            period_series.into_column(),
            factor_series.into_column(),
        ],
    )?;
    Ok(frame.lazy())
}

/// Fit the weight factor for a Parquet cohort and write the factor table.
///
/// Convenience wrapper: scans `input_path`, calls [`fit_weights`], and writes the
/// `(id, period, weight_factor)` table to `output_path` — the exact fixture
/// shape [`crate::expand_weighted_parquet`] consumes.
///
/// # Errors
/// Returns [`ExpandError`] if reading/writing fails, the input path is not valid
/// UTF-8, or fitting fails.
pub fn fit_weights_parquet(
    input_path: impl AsRef<std::path::Path>,
    output_path: impl AsRef<std::path::Path>,
    options: &ExpandOptions,
    spec: &WeightSpec,
) -> Result<()> {
    let input_str = input_path
        .as_ref()
        .to_str()
        .ok_or_else(|| ExpandError::InvalidOptions("input path is not valid UTF-8".to_owned()))?;
    let cohort = LazyFrame::scan_parquet(PlRefPath::new(input_str), ScanArgsParquet::default())?;
    let mut frame = fit_weights(cohort, options, spec)?.collect()?;
    let mut file = std::fs::File::create(output_path)?;
    ParquetWriter::new(&mut file).finish(&mut frame)?;
    Ok(())
}

/// Expand a Parquet cohort, fit its weights, apply them, and write the result.
///
/// The fully in-Rust analogue of [`crate::expand_weighted_parquet`]: instead of
/// reading a pre-computed factor table, it *fits* the factors from `spec`
/// ([`fit_weights`]), then runs [`expand`] → [`apply_weights`]. The output is the
/// six structural columns plus `weight` (Float64). The structural columns are
/// bit-exact; `weight` matches the Oracle within the staged ~1e-6 tolerance.
///
/// # Errors
/// Returns [`ExpandError`] if reading/writing fails, a path is not valid UTF-8,
/// `options` are invalid, fitting fails, or a Polars operation fails.
pub fn expand_weighted_fitted_parquet(
    input_path: impl AsRef<std::path::Path>,
    output_path: impl AsRef<std::path::Path>,
    options: &ExpandOptions,
    spec: &WeightSpec,
) -> Result<()> {
    let input_str = input_path
        .as_ref()
        .to_str()
        .ok_or_else(|| ExpandError::InvalidOptions("input path is not valid UTF-8".to_owned()))?;
    let cohort = LazyFrame::scan_parquet(PlRefPath::new(input_str), ScanArgsParquet::default())?;
    let factors = fit_weights(cohort.clone(), options, spec)?;
    let expanded = expand(cohort, options)?;
    let mut frame = apply_weights(expanded, factors, options)?.collect()?;
    let mut file = std::fs::File::create(output_path)?;
    ParquetWriter::new(&mut file).finish(&mut frame)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! Co-located regression net for the **fitted** weight path: fit each weight
    //! model in Rust (no pre-computed factor table) and assert the weighted frame
    //! matches the Oracle — structural columns bit-exact, `weight` within the
    //! staged ~1e-6 tolerance (ADR-2). The canonical contract test lives in
    //! `tests/weights_fit.rs`. Tolerances live here, never in the fitting code.
    use std::path::{Path, PathBuf};

    use super::{CensorWeightSpec, PoolCensor, SwitchWeightSpec, WeightSpec, fit_weights};
    use crate::{Estimand, ExpandOptions, expand_weighted_fitted_parquet};
    use polars::prelude::*;

    /// Staged tolerance on the fitted `weight` (ADR-2). Observed worst is ~1e-7
    /// (solver-vs-`glm` ~1.6e-8 propagated through the cumulative product); 1e-6
    /// is the documented contract bound, defined here and nowhere in `src/`.
    const FITTED_WEIGHT_REL_TOL: f64 = 1e-6;

    fn fixture(rel: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("fixtures")
            .join(rel)
    }

    fn read(path: &Path) -> DataFrame {
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

    /// PP switching spec used by every switching scenario (`n = ~x2`, `d = ~x2 + x1`).
    fn pp_switch_spec() -> WeightSpec {
        WeightSpec::switching(SwitchWeightSpec::new(["x2"], ["x2", "x1"]))
    }

    fn options(estimand: Estimand) -> ExpandOptions {
        ExpandOptions::new("id", "period", "treatment", 0, i32::MAX).with_estimand(estimand)
    }

    /// Fit-and-apply `input_rel` under `spec`, then assert structural columns are
    /// bit-exact and `weight` matches `expected_rel` within the staged tolerance.
    fn assert_fitted(input_rel: &str, expected_rel: &str, estimand: Estimand, spec: &WeightSpec) {
        let input = fixture(input_rel);
        let expected_path = fixture(expected_rel);
        let out = std::env::temp_dir().join(format!(
            "tte_fitted_{}.parquet",
            expected_rel.replace('/', "_")
        ));
        let opts = options(estimand);
        expand_weighted_fitted_parquet(&input, &out, &opts, spec)
            .expect("fit + weighted expansion");

        let actual = read(&out);
        let expected = read(&expected_path);

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
            let a = actual.column(name).expect("actual col");
            let e = expected.column(name).expect("expected col");
            assert!(
                a.equals(e),
                "[{expected_rel}] structural column '{name}' differs\n{}",
                actual.head(Some(12))
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

    /// The fit must be reproducible bit-for-bit run-to-run (determinism contract):
    /// `smartcore`'s L-BFGS starts from a zero vector with no RNG, so two fits of
    /// the same cohort must produce byte-identical factor tables.
    #[test]
    fn fit_is_deterministic() {
        let input = fixture("weights/input_data_censored.parquet");
        let s = input.to_str().expect("utf8");
        let scan = || {
            LazyFrame::scan_parquet(PlRefPath::new(s), ScanArgsParquet::default()).expect("scan")
        };
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
}
