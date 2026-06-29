# Phase 0 — Scaffold: Completion Summary & Sign-off

**Status: ✅ scaffold complete and verified to compile.** Date: 2026-06-29.

Phase 0's Definition of Done (from
[`../001-initial-ideations/003-project-plan.md`](../001-initial-ideations/003-project-plan.md))
is *"`cargo test` runs and fails cleanly with a clear 'not implemented' message;
fixtures exist with a manifest."* The repository, toolchain, and contract
boundaries are in place and the workspace builds, lints, and tests green. The
one remaining piece — **generating the actual Parquet fixtures from the R
Oracle** — requires the user's pinned R + `TrialEmulation` environment and is
listed under *Remaining human sign-off* below.

## What was built

This was assembled by **two agents working in the same repository in parallel**.

### A. Core scaffold (the Rust + project side)
- **Cargo workspace** (`Cargo.toml`): edition 2024, resolver 3, `[workspace.lints]`
  enforcing `unsafe_code = "forbid"` and bit-exactness denies
  (`float_cmp`, `cast_possible_truncation`, …).
- **`crates/tte-expand/`**: the engine crate — `#![forbid(unsafe_code)]`,
  documented `expand` / `expand_parquet` stubs (`unimplemented!()`), `ExpandError`,
  `ExpandOptions`, and a `tests/itt.rs` fixture-contract skeleton (`#[ignore]`d
  until the engine lands).
- **`bindings/tters/`**: a full extendr R-package scaffold as a **detached** Cargo
  workspace (own lockfile, CRAN-vendorable), path-dep on the core crate.
- **`oracle/`**: the 8 R fixture-generation scripts + README (read-only contract).
- **`docs/001-initial-ideations/`**: the originating research and phased plan,
  relocated to the `###-description/` + numbered-`.md` convention.
- **Tooling**: `rust-toolchain.toml` (1.95.0), `rustfmt.toml`, `clippy.toml`,
  `deny.toml`, `.editorconfig`, `.gitattributes`; CI (`ci.yml`:
  fmt/clippy/test-matrix/check/msrv/deny) + `dependabot.yml`.
- **Top-level**: `README`, `ROADMAP`, `SPEC` (draft), `CONTRIBUTING`, `LICENSE`
  (Apache-2.0, matching upstream), `NOTICE` (credits `Causal-LDA/TrialEmulation`).

### B. Agentic-loop guardrails + repo hygiene (the second agent)
- **`.claude/`**: `settings.json` wiring 5 PreToolUse + 1 PostToolUse hooks;
  6 hook scripts (`protect-immutable-paths`, `block-secrets`, `git-safety-check`,
  `hook-self-protection`, `enforce-project-rules`, `rust-fmt`); 5 rule docs
  (determinism, rust-style, testing, security, git-workflow); a
  `rust-best-practices` skill; a `/check-secrets` command.
- **Secret scanning**: `.gitleaks.toml` (allowlists fixtures/oracle/lockfiles),
  `.github/workflows/secret-scan.yml` (gitleaks 8.30.1, checksum-verified),
  gitleaks + `detect-private-key` in `.pre-commit-config.yaml`.
- **Markdown / YAML lint**: `.markdownlint.yaml` + `.markdownlint-cli2.yaml` +
  `.github/workflows/markdownlint.yml`; `.yamllint.yaml`.
- Augmented `CLAUDE.md` (Rules + Tooling sections) and `.gitignore` (Claude
  local-state ignores).

The `.claude/` hooks operationalise the CLAUDE.md contract: edits to `fixtures/`,
`oracle/`, `tests/`, and `SPEC.md` are blocked; destructive git and secret writes
are blocked; `.rs` files are auto-formatted after each edit.

## Verification performed (2026-06-29)

Against the real, networked toolchain (Rust 1.95.0):

| Check | Result |
|---|---|
| `cargo generate-lockfile` (root + `bindings/tters/src/rust`) | ✅ committed (`Cargo.lock` ×2) |
| `cargo check --workspace --all-features --locked` | ✅ compiles (Polars 0.54.4, 354 crates) |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | ✅ clean |
| `cargo fmt --all --check` | ✅ clean |
| `cargo test --workspace --all-features --all-targets --locked` | ✅ 0 passed, 1 ignored (the contract skeleton) |

## Decisions / deviations recorded during verification

- **Polars `0.54.4`** is the current stable (verified on crates.io) — the earlier
  offline guesses (`0.50` / `0.53`) were superseded.
- **MSRV raised to `1.88`** (from the edition-2024 floor of 1.85): the Polars 0.54
  dependency tree pulls crates (`simd-json`, `sysinfo`) that require Rust 1.88.
  `rust-toolchain.toml`, `clippy.toml`, and the `msrv` CI job were aligned.
- **Polars feature set: `dtype-categorical` instead of `dtype-full`.** Enabling the
  temporal dtypes (`dtype-date`/`datetime`/`time`) activates a broken `Strptime`
  lowering arm in `polars-stream 0.54.4` that fails to compile (it references an
  unimported `IRStringFunction`). The expansion works on integer + categorical
  columns and the input has no datetime columns, so the temporal dtypes are
  deliberately excluded. See the note in `Cargo.toml`.
- **The new streaming engine** (`polars-stream`) is pulled transitively
  (`parquet` → `streaming` → `lazy`) and compiles, but explicit out-of-core
  streaming work is deferred to Phase 5.
- **CI toolchain pitfall fixed**: `rust-toolchain.toml` (1.95.0) was silently
  overriding the `test` matrix and `msrv` jobs; both now force the intended
  toolchain via `RUSTUP_TOOLCHAIN`.

## Remaining human sign-off (gates Phase 1)

These need the user's epidemiology judgement and/or pinned R environment — they
are intentionally **not** done by an agent:

1. **Generate the fixtures.** Run `oracle/run_all.R` against a pinned
   `TrialEmulation` + `renv` to produce `fixtures/**/*.parquet` + `MANIFEST.json`.
2. **Freeze `STRUCTURAL_COLS`** from `names(prep$data)` on that pinned version
   (incl. whether PP emits a censoring/expand-flag column) — see
   `oracle/README.md` → *VERIFY FIRST*.
3. **Author `SPEC.md` §1–§3 + the worked micro-example** from the frozen schema.
4. **Sign off the edge-case catalog** (E04 re-entry, E06 switch-then-back,
   E08 ties, E09 max-fanout) in `oracle/30_edge_cases.R`.
5. **Confirm the Tier-2 tolerances** in `oracle/50_golden_pipeline.R`.

Once (1)–(2) land, remove `#[ignore]` from `crates/tte-expand/tests/itt.rs` and
begin **Phase 1 — ITT expansion**.
