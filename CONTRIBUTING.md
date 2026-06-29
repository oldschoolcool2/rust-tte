# Contributing to tte-expand

Thanks for your interest! This project has an unusual, **contract-first** build
model — please read this before opening a PR.

## The contract boundary

The repository is split into a **read-only contract** and a **writable engine**:

| Path | Role | Editable? |
|---|---|---|
| `crates/tte-expand/src/` | The engine | ✅ yes |
| `crates/tte-expand/tests/` | The contract harness | ⛔ no (tolerances live here) |
| `fixtures/` | Generated Parquet ground truth | ⛔ no (regenerated from the Oracle) |
| `oracle/` | R scripts that generate fixtures | ⛔ no (the Oracle is ground truth) |
| `SPEC.md` | Behavioural orientation | ⛔ no (the fixture wins on conflict) |

If matching a fixture seems to require weakening a test, changing a fixture, or
implementing a statistical solver — **stop and open an issue** instead. That is a
design decision, not a code change. See [`CLAUDE.md`](CLAUDE.md) for the full
operating rules (they apply to humans and agents alike).

## Scope

v1 is **sequential trial emulation only**. The clone-censor-weight (CCW)
grace-period design is explicitly out of scope and belongs in a separate crate.
Statistical estimation (`glm` / `parglm` / `sandwich`) stays in R.

## Local development

```sh
cargo fmt --all                       # format
cargo clippy --all-targets --all-features   # lint (warnings are errors in CI)
cargo test --all-features             # run the fixture-driven suite
cargo deny check                      # licenses / advisories (if cargo-deny installed)
```

A pinned toolchain is provided via [`rust-toolchain.toml`](rust-toolchain.toml);
no manual `rustup` setup should be needed.

## Pull requests

- Keep changes scoped to `crates/tte-expand/src/` unless you are explicitly
  changing tooling, docs, or the Oracle.
- CI must be green: `fmt --check`, `clippy -D warnings`, and the full test suite.
- Explain *why* in the PR description, especially for any new dependency.
- By contributing, you agree your contributions are licensed under
  [Apache-2.0](LICENSE), consistent with the rest of the project.

## Design docs

Substantial design work goes in `docs/` under a new numbered
`###-description/` folder (e.g. `docs/002-spec-freeze/`), with numbered
`###-description.md` files inside. Start from the rationale in
[`docs/001-initial-ideations/`](docs/001-initial-ideations/).
