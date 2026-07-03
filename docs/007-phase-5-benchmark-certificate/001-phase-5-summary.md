# Phase 5 — Benchmark + Reproducibility Certificate: Completion Summary

**Status: ✅ The reproducibility certificate (`make verify`) asserts bit-exact
equivalence to the R `TrialEmulation` Oracle across the full fixture battery —
recomputing every fixture SHA-256 against the manifests and re-verifying
equivalence — and the criterion + R-vs-Rust benchmark surface shows the
runtime/peak-RSS curves, including the regime where R OOMs and Rust does not. The
verified engine and the contract (immutable fixtures, SPEC) were not changed.**
Date: 2026-06-30.

Phase 5's Definition of Done (from [`../../ROADMAP.md`](../../ROADMAP.md)) is
*"Report shows bit-exact equivalence + speed/memory curves; runs in CI."* Two
artifacts deliver it, both reproducible from a single entry point (`make verify`):
the **equivalence certificate** ([`report/certificate.md`](../../report/certificate.md))
and the **benchmark curves** ([`report/benchmark.md`](../../report/benchmark.md)).

## What was implemented (edits confined to allowed Phase-5 paths)

All edits are in `crates/tte-expand/benches/`, `crates/tte-expand/Cargo.toml`
(dev-deps only), `bench/`, `report/`, a top-level `Makefile`, `.github/workflows/`,
and `docs/`. **No change to `crates/tte-expand/src/`, `fixtures/`, `oracle/`,
`tests/`, or `SPEC.md`.**

### A. Reproducibility / equivalence certificate

- **`crates/tte-expand/benches/certificate.rs`** (a `harness = false` bench) —
  generates [`report/certificate.md`](../../report/certificate.md). It does **not**
  hard-code "PASS": it (1) recomputes the SHA-256 of **every** fixture listed in
  `fixtures/MANIFEST.json` + `fixtures/weights/MANIFEST_weights.json` and compares
  to the manifest (differential integrity — **47/47 match**, fails on any drift),
  (2) re-runs the engine on 7 representative fixtures and checks the structural
  columns bit-exact + `weight` within the harness's `1e-12`, and (3) records the
  Oracle provenance (package `0.0.4.11`, R 4.3.3) and the toolchain/dependency
  pins auto-derived from `rust-toolchain.toml`, `Cargo.toml`, `Cargo.lock`, and the
  binding lockfile (rustc 1.95.0, edition 2024 / MSRV 1.95, polars 0.54.4,
  criterion 0.5.1, serde_json 1.0.150, sha2 0.10.9, extendr-api 0.9.0). The
  exhaustive equivalence proof remains the contract suite (`cargo test`), which
  `make verify` runs and which regenerates the certificate as part of the test run.

### B. Benchmark surface

- **`crates/tte-expand/benches/expand.rs`** (criterion) — runtime micro-benchmarks
  of `expand` (ITT / PP) and the weighted `apply_weights` path on seeded synthetic
  inputs; sweep capped by `TTE_BENCH_MAX_ROWS` (default `1e6`) so CI runs a fast
  smoke. **`benches/support.rs`** is the shared, dependency-free, seeded generator
  (a self-contained `SplitMix64`, so inputs are byte-identical across runs).
- **`bench/`** — the R-vs-Rust harness: `gen.R` (fast vectorized seeded generator),
  `prep.R` (times `data_preparation`), `runner/` (a standalone-workspace Rust CLI
  timed via `/usr/bin/time -v`, so R and Rust peak RSS are measured identically),
  `run_bench.sh` (the sweep → `report/benchmark_data.md`), and the Tier-2 golden
  (`golden_roundtrip.R` + `run_golden.sh`).
- **`report/benchmark.md`** — the writeup: runtime + peak-RSS curves vs upstream,
  the criterion curve, and the **R-OOM / Rust-OK** regime.

### C. Entry point + CI

- **`Makefile`** — `verify` (certificate, pure cargo), `bench` / `bench-smoke`
  (criterion), `curves` (R-vs-Rust, manual), `golden` (Tier-2, manual).
- **`.github/workflows/phase-5.yml`** — runs `make verify` (fails on fixture drift
  → differential CI) + `make bench-smoke`, uploads the certificate. Tag-pinned
  actions + least-privilege `permissions: contents: read`, matching the repo's
  policy. (The existing `ci.yml` test job, being `--all-targets`, now also exercises
  the benches and the certificate on stable + beta.)

### D. Dev-dependencies added (justified; all `cargo deny`-clean)

`criterion = "0.5"` (bench harness; relies on the workspace `panic = "unwind"`),
`serde_json = "1"` (parse the Oracle manifests), `sha2 = "0.10"` (recompute fixture
digests with no external tool, keeping `make verify` pure-cargo). `autobenches =
false` so the shared `support.rs` generator is not treated as a bench target.

## VERIFY-FIRST findings (empirical, established before building)

| Unknown | Finding |
|---|---|
| criterion under 1.95.0 | **0.5.1** resolves + compiles under Rust 1.95.0; the `harness=false` bench runs (plotters backend, no gnuplot/system dep). |
| `cargo deny` on the new tree | Installed `cargo-deny 0.18.9`; the full dev-dep tree (criterion + serde_json + sha2) is **`advisories ok, bans ok, licenses ok, sources ok`** — every license already in the allow-list. |
| R baseline timing + RSS | `/usr/bin/time -v` → `Maximum resident set size` is a uniform whole-process method for **both** R and the Rust binary. `data_preparation(estimand_type="ITT", outcome_cov=~1, use_censor_weights=FALSE, separate_files=FALSE, quiet=TRUE)` with a required `data_dir` tempdir. R carries a fixed ~210-220 MiB interpreter/arrow floor (disclosed in the report). |
| Tier-2 golden | `initiators()` runs cleanly + fast (data_censored 0.25 s). **`trial_msm()` accepts the Rust-expanded frame** (exactly `STRUCTURAL_COLS_WEIGHTED`) and reproduces upstream coefficients — the loop is feasible with **zero statistics in Rust**. **In scope** as `make golden`. |
| Tier-3 harvested | `oracle/60_harvest_upstream.R` is a skeleton needing a vendored upstream source tree + `.rds` snapshots — **none present offline**. **Deferred.** |
| Large-N + R-OOM + CI | Deterministic generation of **10⁷ rows** is cheap (10 s, byte-reproducible). R `data_preparation` is impractical at ≈3×10⁶ (~2.5 min) and **OOMs at ≈5×10⁶** under an 8 GiB cap; Rust does the full **10⁷ input (38 M output rows) in 2.5 s / 2.74 GiB** with exact row parity. CI smoke ≤ 1e6 (gen+Rust) / ≤ 1e5 (R); the full sweep + OOM demo are manual. |

## Verification performed (2026-06-30, Rust 1.95.0, R 4.3.3)

| Check | Result |
|---|---|
| `make verify` → certificate | ✅ **47/47** fixtures match manifest; 7/7 live spot-checks pass (weight worst-rel 8.4e-16 / 4.4e-16 ≪ 1e-12) |
| `cargo test --workspace --all-features --all-targets --locked` | ✅ green (lib 39 / itt 17 / pp 17 / weights 5 + benches in test mode), ~35 s |
| `cargo test … --doc --locked` | ✅ |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean (bench-only lint relaxations are scoped + justified) |
| `cargo fmt --all --check` | ✅ |
| `cargo check --workspace --all-targets --all-features --locked` | ✅ |
| `cargo deny check` (criterion + serde_json + sha2) | ✅ advisories / bans / licenses / sources ok |
| `make bench` (criterion ITT/PP/weighted) | ✅ curves produced (ITT → ~8.5 M elem/s) |
| `make curves` (R-vs-Rust) | ✅ e.g. 40.5 k rows: R 2.10 s / 302 MiB vs Rust 0.02 s / 55 MiB |
| `make golden` (Tier-2) | ✅ PASS — estimates agree to ~2e-10, robust SE to ~2e-4 (tol 1e-4 / 1e-3) |
| Lockfiles | root `Cargo.lock` updated (dev-deps); `bindings/tters/src/rust/Cargo.lock` unchanged; `bench/runner/Cargo.lock` committed |

The engine's structural output remains **bit-exact** — the certificate re-proves
it and no `src/` behaviour changed.

## Decisions / deviations recorded

- **Memory measurement = whole-process `/usr/bin/time -v` peak RSS**, identical for
  R and Rust. The R interpreter floor (~215 MiB) is disclosed; both absolute peak
  and the data-scaling slope are reported so the comparison's scope is explicit.
- **Certificate is pure-cargo** (serde_json + sha2, no python/jq/R), so `make
  verify` reproduces it on any platform. It runs as part of `cargo test
  --all-targets`, so the certificate cannot silently go stale.
- **CI vs manual.** CI runs `make verify` + `make bench-smoke` (≤ 1e5). The full
  10⁷ sweep and the R-OOM demonstration are manual/documented (heavy; need R).
- **Tier-2 golden in scope** (`make golden`) — the strongest reproducibility claim
  (Rust-expand → R-estimate → matches upstream) with no Rust statistics.
  **Tier-3 harvested deferred** (needs vendoring the upstream repo + `.rds`).
- **`bench/runner/` is a separate workspace** (own `Cargo.lock`), excluded from the
  root build by construction — keeps `cargo {test,clippy,deny}` at the root
  untouched while still timing the exact verified core via a path dependency.
- **New dev-deps justified + deny-clean**; no `src/`/contract change; no `unsafe`.

## Deferred to later phases

- **Tier-3 harvested upstream** — vendor/pin `Causal-LDA/TrialEmulation` + project
  its expansion-stage `.rds` snapshots to Parquet (a network/human step).
- **Differential CI via Oracle regeneration** — CI currently detects fixture drift
  by SHA-256 (cheap, no R). Regenerating fixtures from the pinned R Oracle on every
  run (the heaviest form of differential CI) stays a documented manual/periodic step.
- **Streaming-sink memory numbers** — the benchmark uses eager `collect()`; the
  lazy/streaming sink would lower peak RSS further at the largest N.
- **Phase 6 (v2)** — weight-model *fitting* in Rust (bind a mature solver); out of
  scope here (the engine does deterministic application only).
