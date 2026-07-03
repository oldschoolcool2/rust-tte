<h1 align="center">tte-expand</h1>

<p align="center">
  <strong>A verified, high-performance Rust&nbsp;+&nbsp;Polars backend for the data-expansion
  stage of sequential target trial emulation.</strong>
</p>

<p align="center">
  <a href="https://github.com/oldschoolcool2/rust-tte/actions/workflows/ci.yml"><img alt="CI" src="https://github.com/oldschoolcool2/rust-tte/actions/workflows/ci.yml/badge.svg"></a>
  <a href="LICENSE"><img alt="License: Apache-2.0" src="https://img.shields.io/badge/license-Apache--2.0-blue.svg"></a>
  <img alt="Status" src="https://img.shields.io/badge/status-phase%200%20scaffolding-orange.svg">
</p>

> **Repository:** `rust-tte` &nbsp;·&nbsp; **Core crate:** `tte-expand` &nbsp;·&nbsp; **R companion:** `tters`

---

## What this is

`tte-expand` is a memory-safe, out-of-core Rust engine that reproduces the
**sequential trial-emulation data expansion** — turning long person-period
observational data into a sequence of nested emulated trials — **bit-for-bit**
identically to the gold-standard R package
[`TrialEmulation`](https://github.com/Causal-LDA/TrialEmulation), and exposes it
back to R users as a drop-in, faster `data_preparation` backend.

The expansion step is the documented scaling wall of the upstream tool (the
maintainers built file-chunking to cope with expanded data that does not fit in
RAM). This project attacks that wall with a Polars lazy/streaming engine while
treating the R package as an **immutable Oracle** for correctness.

### What this is **not**

- ❌ A rewrite of the `TrialEmulation` package, or a fork.
- ❌ A new statistical method, or a reimplementation of `glm` / `parglm` / `sandwich`.
- ❌ The clone-censor-weight (CCW) grace-period design — that is a **separate**
  design and would be a separate crate. v1 is **sequential trial emulation only**
  (Hernán 2008 / Gran 2010 / Danaei 2013).

## Why it's a real contribution (not just "faster")

The deliverable that gives this scientific weight is a **computational-
reproducibility certificate**: a public, reproducible proof of bit-exact
equivalence to the CRAN reference across an adversarial fixture battery. That
artifact speaks directly to the real-world-evidence (RWE) reproducibility
conversation (FDA / EMA / ENCePP) — a *verified* reimplementation is more
citable than a benchmark. Upstream is Apache-2.0 with active maintainers; this
project is a *companion* to their package, not a replacement.

## The approach: fixture-driven strangler pattern

1. **R is the Oracle.** Scripts in [`oracle/`](oracle/) run the upstream package
   on seed/simulated/edge-case cohorts and dump `input_*.parquet` +
   `expected_*.parquet` into [`fixtures/`](fixtures/), with a sha256 manifest.
2. **Rust matches the contract.** The engine in
   [`crates/tte-expand/`](crates/tte-expand/) reads those fixtures and must
   reproduce the structural columns exactly:
   `id, trial_period, followup_time, assigned_treatment, treatment, outcome`
   (plus per-protocol censoring flags).
3. **Staged tolerance.** Deterministic expansion → **exact** (a diff is a bug).
   Anything touching a statistical solver stays in R and is compared only within
   a documented numeric tolerance.

Fixtures are **Parquet, never CSV** (CSV silently coerces int/categorical/NA
typing and round-trips floats, manufacturing false mismatches).

## Repository layout

```
rust-tte/
├── crates/
│   └── tte-expand/         # core Rust + Polars engine (the only place logic lives)
│       ├── src/            # library source
│       └── tests/          # fixture-driven integration tests (the contract)
├── bindings/
│   └── tters/              # R package wrapping the crate via extendr
├── oracle/                 # R scripts that generate fixtures (read-only contract)
├── fixtures/               # generated Parquet fixtures + MANIFEST.json (read-only)
├── bench/                  # criterion benchmarks vs. the R path
├── report/                 # reproducibility certificate + benchmark write-up
├── docs/                   # design docs, numbered ###-description/ folders
│   └── 001-initial-ideations/
├── CLAUDE.md               # operating rules for the agentic build loop
├── SPEC.md                 # R-free behavioural spec of the expansion
└── ROADMAP.md              # phased build plan + definitions of done
```

## Status

✅ **Phase 0 — scaffold complete (2026-06-29).** The repository structure,
tooling, and contract boundaries are in place; the workspace compiles and passes
the strict CI gates (`clippy -D warnings`, `fmt --check`, `cargo test` with the
contract test `#[ignore]`d). The engine itself is the **next** step — its public
entry points are documented `unimplemented!()` stubs. Remaining Phase-0 sign-off
(generating the Parquet fixtures from the R Oracle and freezing `STRUCTURAL_COLS`)
needs the pinned R environment — see the
[Phase-0 summary](docs/002-phase-0-scaffold/001-phase-0-summary.md),
[`ROADMAP.md`](ROADMAP.md), and
[`docs/001-initial-ideations/`](docs/001-initial-ideations/).

## Quickstart

> Requires a recent stable Rust toolchain (pinned via [`rust-toolchain.toml`](rust-toolchain.toml)).

```sh
# Build the workspace (this also generates Cargo.lock on first run)
cargo build

# Run the fixture-driven tests (the engine stub is unimplemented; the contract
# test is #[ignore]d until it lands)
cargo test

# Lint + format checks (also run in CI)
cargo fmt --all --check
cargo clippy --all-targets --all-features
```

> **Toolchain & lockfiles.** The toolchain is pinned via
> [`rust-toolchain.toml`](rust-toolchain.toml) (1.95.0; MSRV 1.95 — Polars 0.54
> requires the latest stable). `Cargo.lock`
> (root) and `bindings/tters/src/rust/Cargo.lock` are committed, and CI runs every
> cargo job with `--locked`. Polars is pinned to `0.54`.
>
> **Optional pre-commit hooks** mirror the CI gates:
> ```sh
> pre-commit install && pre-commit install --hook-type commit-msg
> ```

Regenerating fixtures from the Oracle requires R and the pinned `TrialEmulation`
package; see [`oracle/README.md`](oracle/README.md).

## Relationship to `TrialEmulation` & attribution

This project is built **with respect to**, and validated **against**,
`TrialEmulation` (Causal-LDA; Apache-2.0). The package is used unmodified as the
correctness Oracle. See [`NOTICE`](NOTICE) for full attribution. The intended
contribution pathway is to offer `tters` to the maintainers as a
companion with a `data_preparation`-compatible entry point.

## License

Licensed under the [Apache License, Version 2.0](LICENSE), to match upstream.
See [`NOTICE`](NOTICE) for attribution requirements.
