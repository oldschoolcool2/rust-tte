use extendr_api::prelude::*;
// The extendr `Result` alias (`Result<T, extendr_api::Error>`) is re-exported at
// the crate root but NOT via `prelude`, so import it explicitly — an explicit
// named import shadows std's `Result` in scope.
use extendr_api::Result;

use tte_expand::{
    CensorWeightSpec, Estimand, ExpandOptions, PoolCensor, SwitchWeightSpec, WeightSpec,
};

// Phase 8: in-memory marshalling between an R `data.frame` and a Polars frame, so
// the `*_df` shims below can run cohort-frame -> result-frame with no intermediate
// Parquet (the verified dtype-exact transformation still lives in the core).
mod frame;

/// Map a core [`tte_expand::ExpandError`] to a clean R error condition — the exact
/// mapping the parquet-path shims use, so `ExpandError::WeightFit` and friends
/// surface identically on the in-memory path.
fn core_err(e: tte_expand::ExpandError) -> Error {
    Error::Other(format!("tte-expand: {e}"))
}

/// Map a Polars error (from `.collect()`) to a clean R error condition.
fn collect_err(e: polars::prelude::PolarsError) -> Error {
    Error::Other(format!("tte-expand: {e}"))
}

/// Map an R-supplied estimand label to the core [`Estimand`].
///
/// Accepts `"ITT"` (intention-to-treat) and `"PP"` / `"per-protocol"`,
/// case-insensitively and trimmed. Any other value yields an R error rather than
/// silently defaulting, so a typo at the R boundary fails loudly.
fn parse_estimand(estimand: &str) -> Result<Estimand> {
    match estimand.trim().to_ascii_uppercase().as_str() {
        "ITT" => Ok(Estimand::Itt),
        "PP" | "PER-PROTOCOL" | "PER_PROTOCOL" | "PERPROTOCOL" => Ok(Estimand::PerProtocol),
        other => Err(Error::Other(format!(
            "tte-expand: unknown estimand {other:?}; expected \"ITT\" or \"PP\""
        ))),
    }
}

/// Map an R-supplied `pool_censor` label to the core [`PoolCensor`].
///
/// Accepts `"none"`, `"numerator"` (alias `"num"`) and `"both"`, case-insensitively
/// and trimmed, mirroring `TrialEmulation`'s `pool_cense` choices. Any other value
/// yields an R error rather than silently defaulting.
fn parse_pool(pool: &str) -> Result<PoolCensor> {
    match pool.trim().to_ascii_lowercase().as_str() {
        "none" => Ok(PoolCensor::None),
        "numerator" | "num" => Ok(PoolCensor::Numerator),
        "both" => Ok(PoolCensor::Both),
        other => Err(Error::Other(format!(
            "tte-expand: unknown pool_censor {other:?}; expected \"none\", \"numerator\", or \
             \"both\""
        ))),
    }
}

/// Collect an R character vector (`Strings`) into an owned `Vec<String>`, the
/// shape the core `*Spec` constructors expect for covariate-name lists.
fn covariates(names: &Strings) -> Vec<String> {
    names
        .iter()
        .map(|name| {
            let name: &str = name.as_ref();
            name.to_owned()
        })
        .collect()
}

/// Build the core [`ExpandOptions`] from the flat scalar args R passes.
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn build_options(
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
) -> Result<ExpandOptions> {
    Ok(
        ExpandOptions::new(id_col, period_col, treatment_col, first_period, last_period)
            .with_eligible_col(eligible_col)
            .with_outcome_col(outcome_col)
            .with_estimand(parse_estimand(estimand)?),
    )
}

/// Assemble the nested core [`WeightSpec`] from the flattened R inputs.
///
/// `use_switch` / `use_censor` gate whether each component is present (the R
/// wrapper derives them from non-`NULL` covariate arguments), so an absent
/// component is `None` rather than an empty model. The component constructors are
/// public; `WeightSpec` itself is `#[non_exhaustive]`, so it is assembled through
/// its builder methods (`switching` / `ipcw` / `with_censor`) rather than a struct
/// literal.
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn build_weight_spec(
    use_switch: bool,
    switch_numerator: &Strings,
    switch_denominator: &Strings,
    use_censor: bool,
    censor_col: &str,
    censor_numerator: &Strings,
    censor_denominator: &Strings,
    pool_censor: &str,
) -> Result<WeightSpec> {
    let switch = use_switch.then(|| {
        SwitchWeightSpec::new(covariates(switch_numerator), covariates(switch_denominator))
    });
    let censor = if use_censor {
        Some(CensorWeightSpec::new(
            censor_col,
            covariates(censor_numerator),
            covariates(censor_denominator),
            parse_pool(pool_censor)?,
        ))
    } else {
        None
    };
    Ok(match (switch, censor) {
        (Some(sw), Some(ce)) => WeightSpec::switching(sw).with_censor(ce),
        (Some(sw), None) => WeightSpec::switching(sw),
        (None, Some(ce)) => WeightSpec::ipcw(ce),
        (None, None) => WeightSpec::default(),
    })
}

/// Expand a prepared person-time Parquet dataset into the sequential
/// target-trial layout and write the result to `output_path`.
///
/// This is a thin FFI shim. All dtype-exact, deterministic Polars work lives in
/// the `tte_expand` core crate (which is `#![forbid(unsafe_code)]`). The binding
/// crate cannot forbid unsafe because the extendr macros emit the FFI registrar.
/// Every `tte_expand::ExpandError` is mapped to an R error condition.
///
/// @param input_path Path to the input Parquet file.
/// @param output_path Path where the expanded Parquet is written.
/// @param id_col,period_col,treatment_col Column names in the input.
/// @param eligible_col,outcome_col Eligibility / outcome column names
///   (`TrialEmulation` defaults are `"eligible"` / `"outcome"`).
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` (no artificial censoring) or `"PP"` (per-protocol,
///   censor each trial at the first treatment deviation). Case-insensitive.
/// @return `NULL`, invisibly; the expansion is written to `output_path`. Errors
///   in the core engine surface as R errors.
/// @examples
/// \dontrun{
/// expand_parquet(
///   "input.parquet", "expanded.parquet",
///   "id", "period", "treatment", "eligible", "outcome",
///   0L, .Machine$integer.max, "ITT"
/// )
/// }
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_parquet(
    input_path: &str,
    output_path: &str,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
) -> Result<()> {
    let opts = ExpandOptions::new(id_col, period_col, treatment_col, first_period, last_period)
        .with_eligible_col(eligible_col)
        .with_outcome_col(outcome_col)
        .with_estimand(parse_estimand(estimand)?);
    tte_expand::expand_parquet(input_path, output_path, &opts)
        .map_err(|e| Error::Other(format!("tte-expand: {e}")))?;
    Ok(())
}

/// Expand a person-time Parquet dataset and attach pre-computed
/// inverse-probability weights, writing the weighted frame to `output_path`.
///
/// A thin FFI shim over `tte_expand::expand_weighted_parquet`: it expands the
/// input under `estimand`, joins the per-`(id, period)` factor table at
/// `factors_path` (`id, period, weight_factor`), and writes the six structural
/// columns plus the cumulative-product `weight`. The weight *values* come from R
/// (the `glm` fit); the engine only reproduces their deterministic accumulation.
///
/// @param input_path Path to the input Parquet file.
/// @param factors_path Path to the per-`(id, period)` factor Parquet
///   (`id, period, weight_factor`).
/// @param output_path Path where the weighted Parquet is written.
/// @param id_col,period_col,treatment_col Column names in the input.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`; selects the weight *model* upstream, but the
///   application arithmetic (join + cumulative product) is identical for both.
/// @return `NULL`, invisibly; the weighted expansion is written to `output_path`.
///   Errors in the core engine surface as R errors.
/// @examples
/// \dontrun{
/// expand_weighted_parquet(
///   "input.parquet", "factors.parquet", "weighted.parquet",
///   "id", "period", "treatment", "eligible", "outcome",
///   0L, .Machine$integer.max, "PP"
/// )
/// }
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_weighted_parquet(
    input_path: &str,
    factors_path: &str,
    output_path: &str,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
) -> Result<()> {
    let opts = ExpandOptions::new(id_col, period_col, treatment_col, first_period, last_period)
        .with_eligible_col(eligible_col)
        .with_outcome_col(outcome_col)
        .with_estimand(parse_estimand(estimand)?);
    tte_expand::expand_weighted_parquet(input_path, factors_path, output_path, &opts)
        .map_err(|e| Error::Other(format!("tte-expand: {e}")))?;
    Ok(())
}

/// Fit the inverse-probability **weight factor** for a Parquet cohort in Rust and
/// write the per-`(id, period)` factor table (`id, period, weight_factor`).
///
/// A thin FFI shim over `tte_expand::fit_weights_parquet` (the Phase-6
/// `weights-fit` surface). Unlike `expand_weighted_parquet()`, which *applies* a
/// pre-computed factor table, this *fits* the IPW models in Rust: it ports
/// `TrialEmulation`'s `data_manipulation` + `censor_func` design preparation and
/// binds a deterministic binomial-logit solver for the switching and/or IPCW
/// censoring models, then forms `wt = wt_switch * wtC`. The structural design is
/// exact; the fitted factors reproduce R `glm` within the staged ~1e-6 tolerance
/// (ADR-2), not bit-for-bit. Robust/sandwich variance and the marginal structural
/// model stay in R.
///
/// @param input_path Path to the input Parquet cohort (long person-time).
/// @param output_path Path where the `(id, period, weight_factor)` table is written.
/// @param id_col,period_col,treatment_col Column names in the input.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`; per-protocol runs the artificial-censoring
///   state machine and (with switching covariates) the switch models.
///   Case-insensitive.
/// @param use_switch Whether to fit per-protocol switching-weight models.
/// @param switch_numerator,switch_denominator Covariate columns for the switching
///   numerator (stabiliser) and denominator models (ignored when `use_switch` is
///   `FALSE`).
/// @param use_censor Whether to fit inverse-probability-of-censoring (IPCW) models.
/// @param censor_col Name of the `{0,1}` censoring-indicator column; the response
///   is `1 - censor_col` (ignored when `use_censor` is `FALSE`).
/// @param censor_numerator,censor_denominator Covariate columns for the IPCW
///   numerator/denominator models (ignored when `use_censor` is `FALSE`).
/// @param pool_censor How the IPCW models are pooled across the previous-treatment
///   strata: `"none"`, `"numerator"`, or `"both"`. Case-insensitive.
/// @return `NULL`, invisibly; the factor table is written to `output_path`. Errors
///   in the core engine (including weight-fit failures) surface as R errors.
/// @examples
/// \dontrun{
/// fit_weights_parquet(
///   "cohort.parquet", "factors.parquet",
///   "id", "period", "treatment", "eligible", "outcome",
///   0L, .Machine$integer.max, "PP",
///   TRUE, c("x2"), c("x2", "x1"),
///   FALSE, "", character(0), character(0), "none"
/// )
/// }
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn fit_weights_parquet(
    input_path: &str,
    output_path: &str,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
    use_switch: bool,
    switch_numerator: Strings,
    switch_denominator: Strings,
    use_censor: bool,
    censor_col: &str,
    censor_numerator: Strings,
    censor_denominator: Strings,
    pool_censor: &str,
) -> Result<()> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let spec = build_weight_spec(
        use_switch,
        &switch_numerator,
        &switch_denominator,
        use_censor,
        censor_col,
        &censor_numerator,
        &censor_denominator,
        pool_censor,
    )?;
    tte_expand::fit_weights_parquet(input_path, output_path, &opts, &spec)
        .map_err(|e| Error::Other(format!("tte-expand: {e}")))?;
    Ok(())
}

/// Fit the IPW weights in Rust, expand the cohort, apply the weights, and write
/// the weighted trial frame — a raw cohort to a weighted, expanded frame in one
/// call (no pre-computed factor table).
///
/// A thin FFI shim over `tte_expand::expand_weighted_fitted_parquet`: the fully
/// in-Rust analogue of `expand_weighted_parquet()`. It fits the switching and/or
/// IPCW models from the spec (as `fit_weights_parquet()` does), expands under
/// `estimand`, joins and accumulates the fitted factor, and writes the six
/// structural columns plus the cumulative-product `weight`. Structural columns are
/// bit-exact; `weight` matches the Oracle within the staged ~1e-6 tolerance
/// (ADR-2).
///
/// @param input_path Path to the input Parquet cohort (long person-time).
/// @param output_path Path where the weighted, expanded Parquet is written.
/// @param id_col,period_col,treatment_col Column names in the input.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`. Case-insensitive.
/// @param use_switch Whether to fit per-protocol switching-weight models.
/// @param switch_numerator,switch_denominator Covariate columns for the switching
///   numerator/denominator models (ignored when `use_switch` is `FALSE`).
/// @param use_censor Whether to fit inverse-probability-of-censoring (IPCW) models.
/// @param censor_col Name of the `{0,1}` censoring-indicator column; the response
///   is `1 - censor_col` (ignored when `use_censor` is `FALSE`).
/// @param censor_numerator,censor_denominator Covariate columns for the IPCW
///   numerator/denominator models (ignored when `use_censor` is `FALSE`).
/// @param pool_censor How the IPCW models are pooled across the previous-treatment
///   strata: `"none"`, `"numerator"`, or `"both"`. Case-insensitive.
/// @return `NULL`, invisibly; the weighted expansion is written to `output_path`.
///   Errors in the core engine (including weight-fit failures) surface as R errors.
/// @examples
/// \dontrun{
/// expand_weighted_fitted_parquet(
///   "cohort.parquet", "weighted.parquet",
///   "id", "period", "treatment", "eligible", "outcome",
///   0L, .Machine$integer.max, "PP",
///   TRUE, c("x2"), c("x2", "x1"),
///   FALSE, "", character(0), character(0), "none"
/// )
/// }
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_weighted_fitted_parquet(
    input_path: &str,
    output_path: &str,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
    use_switch: bool,
    switch_numerator: Strings,
    switch_denominator: Strings,
    use_censor: bool,
    censor_col: &str,
    censor_numerator: Strings,
    censor_denominator: Strings,
    pool_censor: &str,
) -> Result<()> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let spec = build_weight_spec(
        use_switch,
        &switch_numerator,
        &switch_denominator,
        use_censor,
        censor_col,
        &censor_numerator,
        &censor_denominator,
        pool_censor,
    )?;
    tte_expand::expand_weighted_fitted_parquet(input_path, output_path, &opts, &spec)
        .map_err(|e| Error::Other(format!("tte-expand: {e}")))?;
    Ok(())
}

/// Expand an in-memory cohort `data.frame` into the sequential target-trial layout
/// and return the result as a `data.frame` — the frame-in/frame-out analogue of
/// `expand_parquet()`, with no intermediate Parquet.
///
/// The cohort arrives as an R `data.frame` (a `list` of equal-length columns);
/// columns are marshalled dtype-exactly into a Polars frame (R `integer` ->
/// `Int32`, `double` -> `Float64`, `bit64::integer64` -> `Int64`), expanded by the
/// verified core, and the six structural columns are marshalled back to an R
/// `data.frame`. A 64-bit integer column (an `integer64`, e.g. a large `id`)
/// round-trips exactly via a pure-safe bit reinterpret (no precision loss).
///
/// @param cohort An R `data.frame` of long person-time rows.
/// @param id_col,period_col,treatment_col Column names in `cohort`.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`. Case-insensitive.
/// @return A `data.frame` with the six structural columns (an `integer64` input
///   column is returned as `integer64`). Errors in the core engine surface as R
///   errors.
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_df(
    cohort: List,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
) -> Result<Robj> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let cohort_lf = frame::lazyframe_from_list(&cohort)?;
    let out = tte_expand::expand(cohort_lf, &opts)
        .map_err(core_err)?
        .collect()
        .map_err(collect_err)?;
    frame::dataframe_to_robj(&out)
}

/// Expand an in-memory cohort and attach pre-computed inverse-probability weights,
/// returning the weighted frame as a `data.frame` — the frame-in/frame-out
/// analogue of `expand_weighted_parquet()`.
///
/// Both the cohort and the per-`(id, period)` factor table (`id, period,
/// weight_factor`) are passed as R `data.frame`s; the engine expands under
/// `estimand`, joins the factors, and accumulates the cumulative-product `weight`.
/// A 64-bit integer `id` (`bit64::integer64`) in either frame round-trips exactly.
///
/// @param cohort An R `data.frame` of long person-time rows.
/// @param factors An R `data.frame` with columns `id`, `period`, `weight_factor`.
/// @param id_col,period_col,treatment_col Column names in `cohort`.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`. Case-insensitive.
/// @return A `data.frame` with the six structural columns plus `weight`. Errors in
///   the core engine surface as R errors.
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_weighted_df(
    cohort: List,
    factors: List,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
) -> Result<Robj> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let cohort_lf = frame::lazyframe_from_list(&cohort)?;
    let factors_lf = frame::lazyframe_from_list(&factors)?;
    let expanded = tte_expand::expand(cohort_lf, &opts).map_err(core_err)?;
    let out = tte_expand::apply_weights(expanded, factors_lf, &opts)
        .map_err(core_err)?
        .collect()
        .map_err(collect_err)?;
    frame::dataframe_to_robj(&out)
}

/// Fit the inverse-probability weight factor for an in-memory cohort and return the
/// per-`(id, period)` factor table (`id, period, weight_factor`) as a `data.frame`
/// — the frame-in/frame-out analogue of `fit_weights_parquet()`.
///
/// @param cohort An R `data.frame` of long person-time rows.
/// @param id_col,period_col,treatment_col Column names in `cohort`.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`. Case-insensitive.
/// @param use_switch Whether to fit per-protocol switching-weight models.
/// @param switch_numerator,switch_denominator Covariate columns for the switching
///   numerator/denominator models (ignored when `use_switch` is `FALSE`).
/// @param use_censor Whether to fit inverse-probability-of-censoring (IPCW) models.
/// @param censor_col Name of the `{0,1}` censoring-indicator column; the response
///   is `1 - censor_col` (ignored when `use_censor` is `FALSE`).
/// @param censor_numerator,censor_denominator Covariate columns for the IPCW
///   numerator/denominator models (ignored when `use_censor` is `FALSE`).
/// @param pool_censor How the IPCW models are pooled across the previous-treatment
///   strata: `"none"`, `"numerator"`, or `"both"`. Case-insensitive.
/// @return A `data.frame` with columns `id`, `period`, `weight_factor` (a 64-bit
///   integer `id` is returned as `bit64::integer64`). Errors in the core engine
///   (including weight-fit failures) surface as R errors.
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn fit_weights_df(
    cohort: List,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
    use_switch: bool,
    switch_numerator: Strings,
    switch_denominator: Strings,
    use_censor: bool,
    censor_col: &str,
    censor_numerator: Strings,
    censor_denominator: Strings,
    pool_censor: &str,
) -> Result<Robj> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let spec = build_weight_spec(
        use_switch,
        &switch_numerator,
        &switch_denominator,
        use_censor,
        censor_col,
        &censor_numerator,
        &censor_denominator,
        pool_censor,
    )?;
    let cohort_lf = frame::lazyframe_from_list(&cohort)?;
    let out = tte_expand::fit_weights(cohort_lf, &opts, &spec)
        .map_err(core_err)?
        .collect()
        .map_err(collect_err)?;
    frame::dataframe_to_robj(&out)
}

/// Fit the IPW weights for an in-memory cohort, expand, apply, and return the
/// weighted trial frame as a `data.frame` — a raw cohort `data.frame` straight to a
/// weighted, expanded `data.frame` in one call (no pre-computed factor table, no
/// intermediate Parquet). The frame-in/frame-out analogue of
/// `expand_weighted_fitted_parquet()`. A 64-bit integer `id` (`bit64::integer64`)
/// round-trips exactly.
///
/// @param cohort An R `data.frame` of long person-time rows.
/// @param id_col,period_col,treatment_col Column names in `cohort`.
/// @param eligible_col,outcome_col Eligibility / outcome column names.
/// @param first_period,last_period Inclusive integer bounds on `trial_period`.
/// @param estimand `"ITT"` or `"PP"`. Case-insensitive.
/// @param use_switch Whether to fit per-protocol switching-weight models.
/// @param switch_numerator,switch_denominator Covariate columns for the switching
///   numerator/denominator models (ignored when `use_switch` is `FALSE`).
/// @param use_censor Whether to fit inverse-probability-of-censoring (IPCW) models.
/// @param censor_col Name of the `{0,1}` censoring-indicator column; the response
///   is `1 - censor_col` (ignored when `use_censor` is `FALSE`).
/// @param censor_numerator,censor_denominator Covariate columns for the IPCW
///   numerator/denominator models (ignored when `use_censor` is `FALSE`).
/// @param pool_censor How the IPCW models are pooled across the previous-treatment
///   strata: `"none"`, `"numerator"`, or `"both"`. Case-insensitive.
/// @return A `data.frame` with the six structural columns plus `weight`. Errors in
///   the core engine (including weight-fit failures) surface as R errors.
/// @export
#[extendr]
#[allow(clippy::too_many_arguments)] // FFI boundary: R passes flat scalar args, not a struct.
fn expand_weighted_fitted_df(
    cohort: List,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    eligible_col: &str,
    outcome_col: &str,
    first_period: i32,
    last_period: i32,
    estimand: &str,
    use_switch: bool,
    switch_numerator: Strings,
    switch_denominator: Strings,
    use_censor: bool,
    censor_col: &str,
    censor_numerator: Strings,
    censor_denominator: Strings,
    pool_censor: &str,
) -> Result<Robj> {
    let opts = build_options(
        id_col,
        period_col,
        treatment_col,
        eligible_col,
        outcome_col,
        first_period,
        last_period,
        estimand,
    )?;
    let spec = build_weight_spec(
        use_switch,
        &switch_numerator,
        &switch_denominator,
        use_censor,
        censor_col,
        &censor_numerator,
        &censor_denominator,
        pool_censor,
    )?;
    let cohort_lf = frame::lazyframe_from_list(&cohort)?;
    // Mirror `expand_weighted_fitted_parquet`: fit the factor from the cohort,
    // expand the same cohort, then apply. LazyFrame clones are cheap (logical plan).
    let factors = tte_expand::fit_weights(cohort_lf.clone(), &opts, &spec).map_err(core_err)?;
    let expanded = tte_expand::expand(cohort_lf, &opts).map_err(core_err)?;
    let out = tte_expand::apply_weights(expanded, factors, &opts)
        .map_err(core_err)?
        .collect()
        .map_err(collect_err)?;
    frame::dataframe_to_robj(&out)
}

// Registers the exported functions with R. The module name here (`tters`) must
// match the package/lib name and the symbols in entrypoint.c / *-win.def.
extendr_module! {
    mod tters;
    fn expand_parquet;
    fn expand_weighted_parquet;
    fn fit_weights_parquet;
    fn expand_weighted_fitted_parquet;
    fn expand_df;
    fn expand_weighted_df;
    fn fit_weights_df;
    fn expand_weighted_fitted_df;
}
