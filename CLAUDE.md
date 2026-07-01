# tte-expand — Agent Operating Rules

> Read this every session. These rules override default behaviour.

## What you are building

A Rust + Polars engine that reproduces, **BIT-FOR-BIT**, the sequential
trial-emulation data expansion from the R package `TrialEmulation`. You are
matching a fixed Oracle output. You are **NOT** doing epidemiology and **NOT**
inventing statistics — you are satisfying a rigid data schema.

## The contract

- **Ground truth = the Parquet fixtures in `fixtures/`.** They are immutable.
- **Orientation = `SPEC.md`.** If `SPEC.md` and a fixture disagree, the
  **FIXTURE WINS** — stop and report the discrepancy with the offending rows.
  Do not silently follow either one.
- **Tolerances are defined in the test harness, not by you.**

## You MAY edit

- `crates/tte-expand/src/` only.

## You MUST NEVER

- Edit `tests/`, `fixtures/`, `oracle/`, or `SPEC.md`.
- Add `#[ignore]`, weaken/skip an assertion, or hard-code a fixture's expected
  values to force a pass.
- Reimplement `glm` / logistic regression / `parglm` / `sandwich` robust
  variance. If a test appears to need a statistical solver, **STOP and report** —
  that is out of scope for v1.
- Add `unsafe` (the crate is `#![forbid(unsafe_code)]`). Add dependencies without
  first explaining why.

## Workflow each iteration

1. Read the ONE failing test + its `SPEC.md` section.
2. Edit `src/`. Run `cargo test <that_test>` (keep the signal tight).
3. On failure, inspect the **actual row-level diff** and revise.
4. On success, run the full suite to check for regressions.
5. Stop when the phase suite is fully green. Report what changed and why.

## When stuck

After ~5 non-improving iterations, **STOP and report**: the failing test, the
smallest reproducing diff, and your hypothesis. Causes are almost always
(a) a typing mismatch (CSV vs Parquet, int vs float, factor ordering),
(b) a `SPEC.md`/fixture conflict, or (c) a genuinely out-of-scope capability.
All three are a human decision, not yours. Do not thrash.

## Scope guardrails

- **v1 is sequential trial emulation ONLY** (Hernán 2008 / Gran 2010 /
  Danaei 2013). It is **NOT** the clone-censor-weight (CCW) grace-period design —
  that is a separate, standalone crate. Never mix the two in one validation loop.
- Rust owns **deterministic data transformation**; R keeps **statistical
  estimation**.

## Definition of done (the current phase is set in the task prompt)

Bit-exact match on the named columns across all named fixtures, all property
tests green, no regressions, no `unsafe`.

## Project conventions

- **Design docs** live in `docs/` under numbered `###-description/` folders, with
  numbered `###-description.md` files inside. The authoritative design rationale
  is in `docs/001-initial-ideations/`.
- **Fixtures are Parquet, never CSV.**
- Use `git mv` (not `mv`) when relocating tracked files, to preserve history.

## Rules (auto-loaded)

Coding-level guidance that backs the contract above. Each is enforced by the
clippy lints in `Cargo.toml`, the test harness, the pre-commit hooks, or the
PreToolUse guards in `.claude/`:

@.claude/rules/determinism.md
@.claude/rules/rust-style.md
@.claude/rules/testing.md
@.claude/rules/security.md
@.claude/rules/git-workflow.md
@.claude/rules/clean-code.md

## Tooling & quality gates

- **Build / test / lint:** `cargo build`, `cargo test` (or `cargo nextest run`),
  `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`,
  `cargo deny check`. (The toolchain is pinned in `rust-toolchain.toml`; if `cargo`
  is missing, install it via `rustup` — formatting hooks degrade silently without it.)
- **Quality gates:** `make clones` (jscpd duplication, `.jscpd.json`) and
  `make antipatterns` (semgrep determinism/anti-pattern scan,
  `.claude/semgrep/agent-antipatterns.yaml`) — run before pushing source
  changes; `/clean-code` runs the full comment-hygiene + duplication audit.
- **Pre-commit:** `pre-commit install && pre-commit install --hook-type commit-msg`,
  then `pre-commit run --all-files`. Runs gitleaks, markdownlint, yamllint, shell
  checks, and a fmt/conventional-commit gate. Never `--no-verify`.
- **CI** (`.github/workflows/`): `ci.yml` (fmt/clippy/test/check/deny +
  reproducibility certificate/bench-smoke; MSRV == the pinned toolchain, no
  separate job), `quality.yml` (jscpd + semgrep), `r-binding.yml` (binding
  fmt/clippy + R installs), `secret-scan.yml` (gitleaks), `markdownlint.yml`.
- **Secret hygiene:** run `/check-secrets` before pushing anything you're unsure
  about; never commit `.env`/keys (the block-secrets guard + gitleaks enforce this).
- **Agent guardrails** (`.claude/settings.json` → `.claude/hooks/`): edits to
  `fixtures/`, `oracle/`, `tests/`, and `SPEC.md` are blocked; destructive git and
  secret writes are blocked; `.rs` files are auto-formatted after each edit.
