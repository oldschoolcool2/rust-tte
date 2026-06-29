#![forbid(unsafe_code)]
//! # tte-expand
//!
//! Verified, high-performance backend for the **data-expansion** stage of
//! *sequential target trial emulation* (epidemiology). It reproduces, bit-for-bit,
//! the expansion output of the R package
//! [`TrialEmulation`](https://cran.r-project.org/package=TrialEmulation)
//! (Apache-2.0) using a Polars lazy/streaming engine with dtype-exact,
//! deterministic integer/categorical handling.
//!
//! Validation is fixture-driven: an R "Oracle" emits Parquet fixtures and this
//! crate must match them exactly (see `tests/itt.rs`).
//!
//! This crate is `#![forbid(unsafe_code)]`. The engine is not yet implemented;
//! the public entry points below are documented stubs.

use std::path::Path;

use polars::prelude::LazyFrame;
use thiserror::Error;

/// Errors returned by the expansion engine.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ExpandError {
    /// A Polars query/IO operation failed.
    #[error("polars error: {0}")]
    Polars(#[from] polars::error::PolarsError),
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
/// new fields can be added without a breaking change.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ExpandOptions {
    /// Subject identifier column.
    pub id_col: String,
    /// Integer period (time-step) column.
    pub period_col: String,
    /// Treatment indicator column.
    pub treatment_col: String,
    /// Inclusive lower period bound.
    pub first_period: i32,
    /// Inclusive upper period bound.
    pub last_period: i32,
}

impl ExpandOptions {
    /// Build a new [`ExpandOptions`].
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
            first_period,
            last_period,
        }
    }
}

/// Expand a prepared person-time [`LazyFrame`] into the sequential
/// target-trial layout.
///
/// # Errors
/// Returns [`ExpandError`] if `options` are invalid or a Polars operation fails.
///
/// # Panics
/// Currently always panics: the expansion engine is not yet implemented.
#[allow(
    clippy::unimplemented,
    clippy::needless_pass_by_value,
    reason = "documented stub; the real lazy/streaming engine lands in a follow-up"
)]
pub fn expand(input: LazyFrame, options: &ExpandOptions) -> Result<LazyFrame> {
    let _ = (&input, options);
    unimplemented!("tte-expand: the sequential trial expansion engine is not yet implemented")
}

/// Read the Parquet file at `input_path`, expand it, and write the dtype-exact
/// result to `output_path`.
///
/// # Errors
/// Returns [`ExpandError`] if reading/writing fails, `options` are invalid, or a
/// Polars operation fails.
///
/// # Panics
/// Currently always panics: the expansion engine is not yet implemented.
#[allow(
    clippy::unimplemented,
    reason = "documented stub; the real Parquet round-trip lands in a follow-up"
)]
pub fn expand_parquet(
    input_path: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
    options: &ExpandOptions,
) -> Result<()> {
    let _ = (input_path.as_ref(), output_path.as_ref(), options);
    unimplemented!("tte-expand: expand_parquet is not yet implemented")
}
