# tte-expand

A verified, high-performance Rust + Polars engine for the **data-expansion stage
of sequential target trial emulation**. It reproduces, bit-for-bit, the expansion
output of the R [`TrialEmulation`](https://github.com/Causal-LDA/TrialEmulation)
package (Apache-2.0), validated fixture-by-fixture against it as an Oracle.

- `#![forbid(unsafe_code)]`, dtype-exact, deterministic integer/categorical output.
- Lazy Polars engine; out-of-core streaming is a later (Phase 5) addition.
- The engine is currently a documented stub — see the crate docs and the
  repository for status.

This crate is part of the [`rust-tte`](https://github.com/oldschoolcool2/rust-tte)
workspace; see the repository root for the full design, roadmap, and the R
companion package `tters`. Licensed under Apache-2.0.
