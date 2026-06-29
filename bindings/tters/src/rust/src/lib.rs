use extendr_api::prelude::*;
// The extendr `Result` alias (`Result<T, extendr_api::Error>`) is re-exported at
// the crate root but NOT via `prelude`, so import it explicitly — an explicit
// named import shadows std's `Result` in scope.
use extendr_api::Result;

use tte_expand::{Estimand, ExpandOptions};

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

// Registers the exported functions with R. The module name here (`tters`) must
// match the package/lib name and the symbols in entrypoint.c / *-win.def.
extendr_module! {
    mod tters;
    fn expand_parquet;
    fn expand_weighted_parquet;
}
