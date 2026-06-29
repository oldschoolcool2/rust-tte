use extendr_api::prelude::*;

/// Expand a prepared person-time Parquet dataset into the sequential
/// target-trial layout and write the result to `output_path`.
///
/// This is a thin FFI shim. All dtype-exact, deterministic Polars work lives in
/// the `tte_expand` core crate (which is `#![forbid(unsafe_code)]`). The binding
/// crate cannot forbid unsafe because the extendr macros emit the FFI registrar.
///
/// @param input_path Path to the input Parquet file.
/// @param output_path Path where the expanded Parquet is written.
/// @param id_col,period_col,treatment_col Column names in the input.
/// @param first_period,last_period Inclusive integer period bounds.
/// @export
#[extendr]
fn expand_parquet(
    input_path: &str,
    output_path: &str,
    id_col: &str,
    period_col: &str,
    treatment_col: &str,
    first_period: i32,
    last_period: i32,
) -> Result<()> {
    let opts = tte_expand::ExpandOptions::new(
        id_col,
        period_col,
        treatment_col,
        first_period,
        last_period,
    );
    tte_expand::expand_parquet(input_path, output_path, &opts)
        .map_err(|e| Error::Other(format!("tte-expand: {e}")))?;
    Ok(())
}

// Registers the exported functions with R. The module name here (`tters`) must
// match the package/lib name and the symbols in entrypoint.c / *-win.def.
extendr_module! {
    mod tters;
    fn expand_parquet;
}

