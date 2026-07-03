# tters 0.1.1

Badge-green maintenance release of the `tters` R binding (no engine changes).

- **ERROR fixed**: the bare `read_expanded_data()` calls in the `te_datastore`
  contract test now resolve via `TrialEmulation::` — `R CMD check` runs testthat
  with `TrialEmulation` installed-but-not-attached, which is why every non-WASM
  r-universe platform job previously failed (#21).
- **WARNING fixed**: `methods` and `utils` are declared in `Imports`, with
  matching `importFrom()` for `methods`, `bit64::integer64` (declaration only —
  the exact int64 round-trip is untouched), and `utils::globalVariables`.
- **NOTE fixed**: `globalVariables(c(":=", "id"))` for the `data.table` NSE
  symbols.

`R CMD check` on the built tarball: **0 errors, 0 warnings, 0 notes**.

# tters 0.1.0

First public release of `tters`, the verified Rust + Polars backend for the
data-expansion stage of sequential target trial emulation, bit-exact against the
R `TrialEmulation` Oracle (see `report/certificate.md`).

- Sequential trial expansion (ITT and per-protocol) with dtype-exact,
  deterministic output.
- Pre-computed IPW application and in-Rust weight fitting (`weights-fit`).
- In-memory `data.frame` round-trip, including exact `bit64::integer64` columns.
- `te_datastore` companion backend for upstream `TrialEmulation` pipelines.
