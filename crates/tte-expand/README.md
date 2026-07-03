# tte-expand

A verified, high-performance Rust + Polars engine for the **data-expansion stage
of sequential target trial emulation**. It reproduces, bit-for-bit, the expansion
output of the R [`TrialEmulation`](https://github.com/Causal-LDA/TrialEmulation)
package (Apache-2.0), validated fixture-by-fixture against it as an Oracle.

- `#![forbid(unsafe_code)]`, dtype-exact, deterministic integer/categorical output.
- Lazy Polars engine covering ITT expansion, per-protocol artificial censoring,
  weight application, and (behind the `weights-fit` feature) in-Rust IPW fitting.
- Verified bit-for-bit against the Oracle; the R companion `tters` ships on
  [r-universe](https://oldschoolcool2.r-universe.dev/tters) (v0.1.1).

This crate is part of the [`rust-tte`](https://github.com/oldschoolcool2/rust-tte)
workspace; see the repository root for the full design, roadmap, and the R
companion package `tters`. Licensed under Apache-2.0.
