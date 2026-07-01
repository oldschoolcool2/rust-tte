# Clean Code (comment hygiene & duplication)

Source comments describe the code as it stands — project chronology lives in
`docs/` and git history, never in source.

## Comments

- **No project-management metadata** in Rust or R source comments: no
  "Phase N", sprint/ticket references, dates, author names, or "added in vX".
  Name the durable thing instead — the feature (`weights-fit`), the module,
  the invariant. The phase narrative belongs in `docs/###-*/` folders and
  commit messages.
- **No positional banner markers** (`# ====…`, `// ----…` section bars).
- **Explain WHY, not WHAT.** Keep domain explanations (epidemiology, IPW,
  dtype contracts), algorithm rationale, and tolerance boundaries — those are
  load-bearing. Delete comments that narrate the next line.
- The binding's `///` docs are **roxygen source**, not rustdoc: rextendr
  copies them into `R/extendr-wrappers.R` and `man/*.Rd`. A prose-only change
  there must be mirrored into those generated files (or regenerate with
  rextendr — expensive; see the memory notes on build cost).

## Duplication — `make clones`

- jscpd (config in `.jscpd.json`) gates at **8% duplicated lines**. Extract
  shared helpers for real duplication; in the engine, any extraction must be
  expression-identical (the bit-for-bit contract) and proven by the fixture
  suite.
- **Accepted residue — do not chase:** flat FFI shim signatures and argument
  forwarding in the tters binding, the per-dtype marshalling arms in
  `frame.rs`, and the explicit `arg = arg` pass-through of the R user-facing
  wrappers. That is deliberate API-boundary explicitness.
- **Anti-hydra:** if extraction fragments code into pieces harder to read
  than the duplication, keep the duplication.

## Anti-patterns — `make antipatterns`

- The semgrep ruleset (`.claude/semgrep/agent-antipatterns.yaml`) enforces
  the path-scoped bans clippy cannot express: no wall-clock, env reads, RNG,
  hash-ordered collections, or stdout/stderr prints in the transform path
  (`src/` of engine and binding); no `#![deny(warnings)]`; no crate-level
  `#![allow(clippy::…)]` in library source; no borrow-checker-appeasing
  `&mut x.clone()`.
- A genuine false positive is fixed by adjusting the **rule** (with
  justification, same PR) — never by sneaking the pattern past the scan.

Run both targets before pushing changes that touch source; CI
(`quality.yml`) runs them on every PR and a finding fails the build.
`/clean-code` re-runs the full audit.
