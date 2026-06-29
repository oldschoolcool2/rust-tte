# Rust Coding Style

Match the surrounding code first; these are the defaults when there's no local
precedent.

## Safety & lints
- The engine crate is `#![forbid(unsafe_code)]`. Never add `unsafe`. If something
  seems to require it, STOP and report.
- Code must be **`cargo clippy --all-targets --all-features -- -D warnings` clean**,
  including `clippy::pedantic` (warnings are enabled at the workspace level). Fix
  the lint; reach for a scoped `#[allow(...)]` with a one-line justification only
  when the lint is genuinely wrong for that site.
- Keep `cargo fmt` clean — the PostToolUse hook runs `rustfmt` on every edit.

## Error handling
- Return `Result<T, E>`; propagate with `?`. Reserve `panic!` for truly
  unreachable invariants, and document why.
- **No `.unwrap()` / `.expect()` in library code paths.** In tests they're fine.
  When an invariant guarantees success, prefer `expect("why this cannot fail")`
  over a bare `unwrap()` and explain the invariant.
- Library/engine errors: a typed enum via `thiserror`. Reserve `anyhow` for
  binary/glue code, never in the public API of `tte-expand`.
- On a fixture/spec mismatch, surface a precise, row-level error — do not paper
  over it (see [[determinism]] and CLAUDE.md's "fixture wins" rule).

## Idioms
- Prefer iterator chains over index loops; borrow (`&T`/`&[T]`) over cloning;
  clone deliberately, not reflexively.
- Make illegal states unrepresentable — newtypes and enums over loose `bool`/
  `String` flags.
- Public items get `///` doc comments with a runnable example where it clarifies use.
- Be explicit about integer/float types at data boundaries (CSV vs Parquet,
  `i32` vs `i64`, `f32` vs `f64`) — silent coercion is a top cause of fixture
  mismatches.

## Polars
- Prefer the lazy API (`LazyFrame`) and let the optimizer fuse operations.
- Set column dtypes **explicitly**; never rely on type inference for schema-bearing
  data. The output schema must match the Oracle exactly.
- Use stable, fully-specified sorts (see [[determinism]]) — never depend on input
  or hash-map iteration order.

## Dependencies
- Don't add a dependency without explaining why (CLAUDE.md). New deps must pass
  `cargo deny check` (license + advisory + bans). Prefer the std library and
  already-present crates.
