---
description: Clean-code audit — comment hygiene (no PM metadata/banners/debug prints), jscpd duplication, and the semgrep antipattern scan; investigation first, fixes only after approval.
allowed-tools: Bash(grep *), Bash(make clones*), Bash(make antipatterns*), Bash(uvx semgrep *), Bash(npx *), Bash(cargo *), Bash(git diff*), Bash(git status*), Read, Grep, Glob, Edit
---

# /clean-code

Audit the codebase against `.claude/rules/clean-code.md` and fix violations.
**Phases 1–2 are read-only; do not edit anything before the user approves the
plan in Phase 3.**

## Scope

Editable sources only: `crates/tte-expand/src`, `crates/tte-expand/benches`,
`bindings/tters/src/rust/src`, `bindings/tters/R`, `bindings/tters/tools`,
`bench`. Never `tests/`, `fixtures/`, `oracle/`, `SPEC.md` (immutable), nor
generated files (`extendr-wrappers.R`, `document.rs`, `man/*.Rd`) — except to
mirror a roxygen prose change into its generated copies.

## Phase 1 — Discovery (read-only)

1. Comment hygiene greps over the scope above (`*.rs`, `*.R`):
   - PM metadata: `(//|#).*([Pp]hase [0-9]+|[Ss]print|[Tt]icket|JIRA-|[Ff]ixes #|[Aa]dded by|20[0-9]{2}-[0-9]{2}-[0-9]{2})`
   - Banner markers: `^\s*(//|#)\s*[=\-#*]{10,}`
   - Debug output in library src: `println!|eprintln!|dbg!` (Rust src),
     `^\s*(print|cat)\(` (R, excluding show/print S4 methods)
   - Commented-out code: `^\s*//\s*(let |fn |use |if |return )` and
     `^\s*#\s*[a-zA-Z_.]+\s*(<-|\()` (R, excluding `#'` roxygen)
2. `make clones` — duplication against the 8% gate.
3. `make antipatterns` — the semgrep determinism/anti-pattern scan.

## Phase 2 — Report (read-only)

Categorize every hit: fix / whitelisted (domain explanation, legitimate TODO,
accepted-duplication residue, S4 `show` output) / immutable-path (report
only). Present the full plan with file:line previews and STOP for approval.

## Phase 3 — Fix (after explicit approval)

- Comment rewrites: name the durable feature/invariant instead of chronology.
- Roxygen changes: mirror the identical prose edit into `extendr-wrappers.R`
  and the affected `man/*.Rd`.
- Dedup in the engine must be expression-identical (bit-for-bit contract).
- If `certificate.rs` report strings change, regenerate via `make verify` in
  the same commit and confirm the report diff is wording-only.

## Phase 4 — Validate

`cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D
warnings`, `cargo test --all-features`, `make clones`, `make antipatterns`;
for binding Rust changes also `cargo clippy --all-targets --locked -- -D
warnings` in `bindings/tters/src/rust` (R round-trip validation is CI's job —
local disk cannot hold a second Polars build tree). Report what changed, what
was whitelisted, and any immutable-path findings needing a human decision.
