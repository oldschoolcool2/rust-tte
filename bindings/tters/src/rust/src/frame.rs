//! Phase 8 — in-memory marshalling between an R `data.frame` and a Polars frame.
//!
//! The parquet-path shims (`expand_parquet`, …) hand data over via files. This
//! module lets the `*_df` shims take an R `data.frame` straight to a Polars
//! `DataFrame`/`LazyFrame` and return the result as an R `data.frame`, with NO
//! intermediate Parquet on the critical path. All dtype-exact, deterministic
//! transformation still happens in the verified `tte_expand` core; this module
//! only moves columns across the FFI boundary.
//!
//! ## The dtype contract
//! The whole battery's structural columns are only ever `Int32` or `Float64`
//! (verified against the fixtures: `id`/`treatment`/`assigned_treatment` pass the
//! input dtype through and are int32 or double; `trial_period`/`followup_time` are
//! Int32; `outcome`/`weight` are Float64). Base R represents both natively
//! (`integer` / `double`), so a column-wise mapping is **dtype-exact** with no
//! extra dependencies and full control of the int-vs-double boundary:
//!
//! | direction | R | Polars |
//! |---|---|---|
//! | in  | `integer`   | `Int32`   |
//! | in  | `double`    | `Float64` |
//! | in  | `logical`   | `Boolean` |
//! | in  | `character` | `String`  |
//! | out | `Int32`     | `integer` |
//! | out | `Float64`   | `double`  |
//! | out | `Boolean`   | `logical` |
//! | out | `String`    | `character` |
//!
//! This reproduces, bit-for-bit, what `scan_parquet`/`ParquetWriter` already do on
//! the parquet path (Arrow `int32`↔R `integer`, `double`↔R `double`), so the
//! in-memory path matches the same fixtures at the same tolerances.
//!
//! ## Where exactness ends
//! Base R has no native 64-bit integer, so `Int64`/`UInt32`/`UInt64` columns have
//! no exact base-R representation — those raise a clear error rather than silently
//! widening to `double` (none occur in the validated battery; an Arrow/`bit64`
//! bridge is the documented follow-up). On input, an R `integer64` (`bit64`) or
//! `factor` column is likewise rejected loudly: reading its storage as plain
//! doubles/codes would be a silent correctness bug.

use extendr_api::Result;
use extendr_api::na::CanBeNA;
use extendr_api::prelude::*;
use polars::prelude::{
    BooleanChunked, Column, DataFrame, DataType, Float64Chunked, Int32Chunked, IntoColumn,
    IntoLazy, IntoSeries, LazyFrame, NewChunkedArray, PlSmallStr, Series, StringChunked,
};

/// Map a Polars error to a clean R error condition, matching the shim convention.
fn polars_err(e: polars::prelude::PolarsError) -> Error {
    Error::Other(format!("tte-expand: {e}"))
}

/// Whether `obj` carries `class` among its R class attribute.
fn has_class(obj: &Robj, class: &str) -> bool {
    obj.class()
        .is_some_and(|mut classes| classes.any(|c| c == class))
}

/// Build a single Polars [`Series`] from one named R column, preserving the
/// int-vs-double contract and mapping R `NA` to a Polars null.
fn series_from_column(name: &str, value: &Robj) -> Result<Series> {
    let pl_name: PlSmallStr = name.into();
    match value.rtype() {
        Rtype::Integers => {
            // An R `factor` is an INTSXP of level codes; reading it as plain Int32
            // would silently substitute codes for values. Reject it loudly.
            if has_class(value, "factor") {
                return Err(Error::Other(format!(
                    "tte-expand: column '{name}' is a factor; convert it to numeric or character \
                     before the in-memory path"
                )));
            }
            let slice = value.as_integer_slice().ok_or_else(|| {
                Error::Other(format!("tte-expand: column '{name}' is not integer"))
            })?;
            let ca = Int32Chunked::from_iter_options(
                pl_name,
                slice
                    .iter()
                    .map(|&x| if x.is_na() { None } else { Some(x) }),
            );
            Ok(ca.into_series())
        },
        Rtype::Doubles => {
            // `bit64::integer64` is stored as a REALSXP whose bits are i64, not an
            // IEEE double — reading it as f64 would corrupt every value silently.
            if has_class(value, "integer64") {
                return Err(Error::Other(format!(
                    "tte-expand: column '{name}' is integer64 (bit64); not supported by the \
                     in-memory path — use the parquet path or cast to integer/double"
                )));
            }
            let slice = value.as_real_slice().ok_or_else(|| {
                Error::Other(format!("tte-expand: column '{name}' is not double"))
            })?;
            let ca = Float64Chunked::from_iter_options(
                pl_name,
                slice
                    .iter()
                    .map(|&x| if x.is_na() { None } else { Some(x) }),
            );
            Ok(ca.into_series())
        },
        Rtype::Logicals => {
            let slice = value.as_logical_slice().ok_or_else(|| {
                Error::Other(format!("tte-expand: column '{name}' is not logical"))
            })?;
            let ca = BooleanChunked::from_iter_options(
                pl_name,
                slice
                    .iter()
                    .map(|b| if b.is_na() { None } else { Some(b.is_true()) }),
            );
            Ok(ca.into_series())
        },
        Rtype::Strings => {
            let iter = value.as_str_iter().ok_or_else(|| {
                Error::Other(format!("tte-expand: column '{name}' is not character"))
            })?;
            let ca = StringChunked::from_iter_options(
                pl_name,
                iter.map(|s| if s.is_na() { None } else { Some(s) }),
            );
            Ok(ca.into_series())
        },
        other => Err(Error::Other(format!(
            "tte-expand: column '{name}' has unsupported R type {other:?}; expected integer, \
             double, logical, or character"
        ))),
    }
}

/// Convert an R `data.frame` (received as a `List` of equal-length columns) into a
/// Polars [`LazyFrame`] for the core engine. Column names are taken from the
/// list's `names`; dtypes follow the in-direction table above.
///
/// # Errors
/// Returns an R error if a column has an unsupported R type, the columns have
/// unequal lengths, or a Polars frame cannot be assembled.
pub(crate) fn lazyframe_from_list(cohort: &List) -> Result<LazyFrame> {
    let mut columns: Vec<Column> = Vec::with_capacity(cohort.len());
    let mut height: Option<usize> = None;
    for (name, value) in cohort.iter() {
        let series = series_from_column(name, &value)?;
        match height {
            Some(h) if h != series.len() => {
                return Err(Error::Other(format!(
                    "tte-expand: column '{name}' has length {} but the frame has {h} rows",
                    series.len()
                )));
            },
            _ => height = Some(series.len()),
        }
        columns.push(series.into_column());
    }
    let df = DataFrame::new(height.unwrap_or(0), columns).map_err(polars_err)?;
    Ok(df.lazy())
}

/// Build one R column vector from a Polars [`Column`], preserving the out-direction
/// dtype contract and mapping Polars nulls to R `NA`.
fn column_to_robj(name: &str, col: &Column) -> Result<Robj> {
    let robj = match col.dtype() {
        DataType::Int32 => col.i32().map_err(polars_err)?.iter().collect_robj(),
        DataType::Float64 => col.f64().map_err(polars_err)?.iter().collect_robj(),
        DataType::Boolean => col.bool().map_err(polars_err)?.iter().collect_robj(),
        DataType::String => col.str().map_err(polars_err)?.iter().collect_robj(),
        // Narrower integers / Float32 widen losslessly into base R's int/double.
        DataType::Int8 | DataType::Int16 | DataType::UInt8 | DataType::UInt16 => col
            .cast(&DataType::Int32)
            .map_err(polars_err)?
            .i32()
            .map_err(polars_err)?
            .iter()
            .collect_robj(),
        DataType::Float32 => col
            .cast(&DataType::Float64)
            .map_err(polars_err)?
            .f64()
            .map_err(polars_err)?
            .iter()
            .collect_robj(),
        other => {
            return Err(Error::Other(format!(
                "tte-expand: result column '{name}' has dtype {other:?}, which has no exact base R \
                 representation (Int64/UInt32/UInt64 need the parquet path or an Arrow bridge)"
            )));
        },
    };
    Ok(robj)
}

/// Convert a Polars [`DataFrame`] into an R `data.frame`.
///
/// Each column is marshalled per the out-direction dtype table; the result is a
/// `list` carrying `class = "data.frame"` and the compact automatic
/// `row.names = c(NA, -nrow)` (so `nrow()` and friends work without materialising
/// an `1:n` index — important at scale).
///
/// # Errors
/// Returns an R error if a column dtype has no exact base R representation, the
/// frame has more than `i32::MAX` rows, or attribute assignment fails.
pub(crate) fn dataframe_to_robj(df: &DataFrame) -> Result<Robj> {
    let nrow = df.height();
    let nrow_i32 = i32::try_from(nrow).map_err(|_| {
        Error::Other(format!(
            "tte-expand: result has {nrow} rows, too many for a base R data.frame (max {})",
            i32::MAX
        ))
    })?;

    let mut names: Vec<String> = Vec::with_capacity(df.width());
    let mut values: Vec<Robj> = Vec::with_capacity(df.width());
    for col in df.columns() {
        let name = col.name().to_string();
        values.push(column_to_robj(&name, col)?);
        names.push(name);
    }

    let mut robj: Robj =
        List::from_names_and_values(names.iter().map(String::as_str), values)?.into();
    robj.set_class(&["data.frame"])?;
    // Compact automatic row names: the integer vector c(NA_integer_, -nrow). R
    // stores i32::MIN as NA, so this is exactly `.set_row_names(nrow)`.
    robj.set_attrib(
        Symbol::from_string("row.names"),
        Robj::from(vec![i32::MIN, -nrow_i32]),
    )?;
    Ok(robj)
}
