<!-- Thanks for contributing! See CONTRIBUTING.md and CLAUDE.md for the full
     contract-first rules. Keep the checklist; delete this comment. -->

## What & why

<!-- What does this change, and why? Link any related issue. -->

## Checklist

- [ ] Changes are scoped to `crates/tte-expand/src/` (or clearly tooling, docs, the R binding, or the Oracle).
- [ ] I did **not** edit `tests/`, `fixtures/`, `oracle/`, or `SPEC.md` to force a pass — the fixture wins on any conflict.
- [ ] CI is green: `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -D warnings`, `cargo test --all-features`.
- [ ] Any new dependency is justified below and passes `cargo deny check`.
- [ ] For behaviour changes, I explained the **why**, not just the what.
