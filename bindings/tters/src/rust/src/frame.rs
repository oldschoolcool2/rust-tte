//! Phase 8/9 — in-memory marshalling between an R `data.frame` and a Polars frame.
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
//! | in  | `integer`            | `Int32`   |
//! | in  | `double`             | `Float64` |
//! | in  | `integer64` (bit64)  | `Int64`   |
//! | in  | `logical`            | `Boolean` |
//! | in  | `character`          | `String`  |
//! | out | `Int32`              | `integer` |
//! | out | `Float64`            | `double`  |
//! | out | `Int64`/`UInt32`/`UInt64` | `integer64` (bit64) |
//! | out | `Boolean`            | `logical` |
//! | out | `String`             | `character` |
//!
//! For the `Int32`/`Float64` columns this reproduces, bit-for-bit, what
//! `scan_parquet`/`ParquetWriter` already do on the parquet path (Arrow
//! `int32`↔R `integer`, `double`↔R `double`), so the in-memory path matches the
//! same fixtures at the same tolerances.
//!
//! ## Exact 64-bit integers (Phase 9)
//! Base R has no native 64-bit integer; the `bit64` package stores one in a
//! `REALSXP` whose 8 bytes per element **are** the `i64` bit pattern (NOT an IEEE
//! double), tagged with `class = "integer64"`, using `i64::MIN`'s bit pattern as
//! its `NA` sentinel. Phase 9 round-trips these **exactly**, both directions, by
//! reinterpreting the bits with pure-safe std (`f64::to_ne_bytes` /
//! `i64::from_ne_bytes`) — NO `unsafe`, NO Arrow C Data Interface. Both halves run
//! in the same process, so native-endian byte order is correct on both sides.
//! A naive numeric cast (`x as f64` / `v as i64`) would silently lose precision
//! above `2^53`; the bit-reinterpret preserves every `i64` value.
//!
//! ## Where exactness still ends
//! A `factor` (an INTSXP of level codes) is rejected loudly on input: reading its
//! storage as plain `Int32` would substitute codes for values. A `UInt64` value
//! beyond `i64::MAX` cannot fit `integer64` (which is signed) and likewise errors
//! rather than wrapping to a negative `i64`.

use extendr_api::Result;
use extendr_api::na::CanBeNA;
use extendr_api::prelude::*;
use polars::prelude::{
    BooleanChunked, Column, DataFrame, DataType, Float64Chunked, Int32Chunked, Int64Chunked,
    IntoColumn, IntoLazy, IntoSeries, LazyFrame, NewChunkedArray, PlSmallStr, Series,
    StringChunked,
};

/// `bit64`'s `NA` sentinel: the `integer64` whose bit pattern is `i64::MIN`. Maps
/// to/from a Polars null at the FFI boundary.
const BIT64_NA: i64 = i64::MIN;

/// Map a Polars error to a clean R error condition, matching the shim convention.
fn polars_err(e: polars::prelude::PolarsError) -> Error {
    Error::Other(format!("tte-expand: {e}"))
}

/// Whether `obj` carries `class` among its R class attribute.
fn has_class(obj: &Robj, class: &str) -> bool {
    obj.class()
        .is_some_and(|mut classes| classes.any(|c| c == class))
}

/// Reinterpret an `f64`'s bit pattern as the `i64` with the same bits — recovering
/// a `bit64::integer64` value from its `REALSXP` storage. The exact inverse of
/// `i64_to_f64_bits`; a pure-safe bitcast (NOT a numeric cast), so values above
/// `2^53` survive losslessly. Same-process FFI ⇒ native-endian is correct.
#[inline]
fn f64_bits_to_i64(bits: f64) -> i64 {
    i64::from_ne_bytes(bits.to_ne_bytes())
}

/// Reinterpret an `i64`'s bit pattern as the `f64` with the same bits — `bit64`'s
/// `integer64` storage encoding (NOT a numeric cast). The exact inverse of
/// `f64_bits_to_i64`.
#[inline]
fn i64_to_f64_bits(value: i64) -> f64 {
    f64::from_ne_bytes(value.to_ne_bytes())
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
            // `bit64::integer64` is a REALSXP whose 8 bytes per element ARE an i64
            // (NOT an IEEE double): class "integer64", NA = i64::MIN's bit pattern.
            // Recover each i64 by reinterpreting the stored f64's bits (pure-safe;
            // no `unsafe`, no Arrow C interface), mapping the NA sentinel to a null.
            if has_class(value, "integer64") {
                let slice = value.as_real_slice().ok_or_else(|| {
                    Error::Other(format!("tte-expand: column '{name}' is not a real vector"))
                })?;
                let ca = Int64Chunked::from_iter_options(
                    pl_name,
                    slice.iter().map(|&bits| {
                        let v = f64_bits_to_i64(bits);
                        (v != BIT64_NA).then_some(v)
                    }),
                );
                return Ok(ca.into_series());
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
             double, integer64, logical, or character"
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

/// Build an R `bit64::integer64` column from `i64` values: reinterpret each `i64`
/// to the `f64` carrying its bit pattern, collect into a `REALSXP`, and tag it
/// `class = "integer64"`. A Polars null becomes `bit64`'s `NA` (`i64::MIN`).
///
/// The values MUST be collected as `f64` (the bit-reinterpreted values): extendr's
/// `ToVectorValue for i64` would instead apply a **lossy** numeric `as f64` cast,
/// silently corrupting values above `2^53` — exactly what this path avoids.
fn integer64_robj<I: Iterator<Item = Option<i64>>>(values: I) -> Result<Robj> {
    let mut robj = values
        .map(|v| i64_to_f64_bits(v.unwrap_or(BIT64_NA)))
        .collect_robj();
    robj.set_class(&["integer64"])?;
    Ok(robj)
}

/// Build one R column vector from a Polars [`Column`], preserving the out-direction
/// dtype contract and mapping Polars nulls to R `NA`.
fn column_to_robj(name: &str, col: &Column) -> Result<Robj> {
    let robj = match col.dtype() {
        DataType::Int32 => col.i32().map_err(polars_err)?.iter().collect_robj(),
        DataType::Float64 => col.f64().map_err(polars_err)?.iter().collect_robj(),
        DataType::Boolean => col.bool().map_err(polars_err)?.iter().collect_robj(),
        DataType::String => col.str().map_err(polars_err)?.iter().collect_robj(),
        // 64-bit integers have no native base-R type; carry them exactly as a
        // `bit64::integer64` (see `integer64_robj`). `UInt32` always fits `i64`.
        DataType::Int64 => return integer64_robj(col.i64().map_err(polars_err)?.iter()),
        DataType::UInt32 => {
            return integer64_robj(
                col.u32()
                    .map_err(polars_err)?
                    .iter()
                    .map(|o| o.map(i64::from)),
            );
        },
        DataType::UInt64 => {
            // `integer64` is signed; a value beyond `i64::MAX` cannot be represented
            // and must fail loudly rather than wrap to a negative i64.
            let ca = col.u64().map_err(polars_err)?;
            let values: Vec<Option<i64>> = ca
                .iter()
                .map(|o| {
                    o.map(|v| {
                        i64::try_from(v).map_err(|_| {
                            Error::Other(format!(
                                "tte-expand: result column '{name}' is UInt64 value {v}, which \
                                 exceeds i64::MAX and has no R integer64 representation"
                            ))
                        })
                    })
                    .transpose()
                })
                .collect::<Result<Vec<Option<i64>>>>()?;
            return integer64_robj(values.into_iter());
        },
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
                 representation"
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

#[cfg(test)]
mod tests {
    //! Pure-Rust coverage of the `bit64::integer64` bit-reinterpret (no R runtime).
    //! The full frame-in/frame-out round-trip (incl. the REALSXP `class` tag and
    //! the staged-tolerance battery) is exercised by the `testthat` suite against
    //! the installed package; here we lock the bit-exact reinterpret contract that
    //! a naive numeric cast would break above `2^53`.
    use super::{BIT64_NA, f64_bits_to_i64, i64_to_f64_bits};

    /// Decimal values used by the empirical R verification (`bit64`), incl. the
    /// precision-critical `2^53 + 1` (not representable as an IEEE double).
    const POW2_53: i64 = 9_007_199_254_740_992; // 2^53
    const POW2_53_PLUS_1: i64 = 9_007_199_254_740_993; // 2^53 + 1 (not f64-exact)

    #[test]
    fn reinterpret_round_trips_every_representative_i64() {
        for v in [
            0_i64,
            1,
            -1,
            2,
            POW2_53,
            POW2_53_PLUS_1,
            -POW2_53_PLUS_1,
            i64::MAX,
            i64::MIN + 1, // smallest non-NA value
            i64::MIN,     // the bit64 NA sentinel pattern itself
        ] {
            assert_eq!(
                f64_bits_to_i64(i64_to_f64_bits(v)),
                v,
                "bit-reinterpret must round-trip {v} exactly"
            );
        }
    }

    #[test]
    fn precision_critical_values_are_distinct_unlike_a_naive_cast() {
        // A numeric cast would collapse 2^53 and 2^53+1 to the same f64; the
        // bit-reinterpret keeps them distinct (the whole point of Phase 9).
        let a = i64_to_f64_bits(POW2_53);
        let b = i64_to_f64_bits(POW2_53_PLUS_1);
        assert_ne!(
            a.to_bits(),
            b.to_bits(),
            "distinct i64 must keep distinct bits"
        );
        assert_eq!(f64_bits_to_i64(a), POW2_53);
        assert_eq!(f64_bits_to_i64(b), POW2_53_PLUS_1);
        // Sanity: a naive numeric cast really does lose the low bit here.
        assert_eq!(POW2_53 as f64, POW2_53_PLUS_1 as f64);
    }

    #[test]
    fn na_sentinel_is_i64_min() {
        // bit64's NA is i64::MIN; its f64 bit pattern is the sign bit only
        // (0x8000_0000_0000_0000 == -0.0), and it must reinterpret back to i64::MIN.
        assert_eq!(BIT64_NA, i64::MIN);
        assert_eq!(i64_to_f64_bits(BIT64_NA).to_bits(), 0x8000_0000_0000_0000);
        assert_eq!(f64_bits_to_i64(i64_to_f64_bits(BIT64_NA)), i64::MIN);
    }
}
