---
name: rust-best-practices
description: Rust + Polars best practices for the tte-expand engine — idiomatic error handling, ownership, performance, and deterministic data transformation. Triggers when writing, reviewing, or refactoring Rust code in crates/, designing data pipelines, choosing dependencies, or chasing a fixture mismatch. Reinforces the determinism / rust-style / testing rules with concrete patterns.
---

# Rust Best Practices (tte-expand)

A working guide for writing correct, idiomatic, **deterministic** Rust for the
trial-emulation expansion engine. This is the "how"; the non-negotiables live in
`.claude/rules/` and `CLAUDE.md`.

## When to Apply

- Writing or refactoring code under `crates/*/src/`
- Designing a Polars transformation pipeline
- Reviewing a diff for correctness, clippy-cleanliness, or nondeterminism
- Evaluating whether to add a dependency
- Diagnosing a bit-exact fixture mismatch

## 1. Correctness & Error Handling (CRITICAL)

**err-result-not-panic** — Return `Result`, propagate with `?`. No `.unwrap()` /
`.expect()` on the engine path.

```rust
// Bad — panics on malformed fixture input, no context
let n: i64 = field.parse().unwrap();

// Good — typed error, row context, ? propagation
let n: i64 = field
    .parse()
    .map_err(|e| ExpandError::ParseInt { row, column: "period", source: e })?;
```

**err-thiserror-libs** — Library errors are a typed `enum` via `thiserror`; keep
`anyhow` out of `tte-expand`'s public API.

```rust
#[derive(Debug, thiserror::Error)]
pub enum ExpandError {
    #[error("schema mismatch in {column}: expected {expected}, got {actual}")]
    Schema { column: String, expected: String, actual: String },
    #[error("fixture/spec conflict at row {row}: {detail}")]
    Conflict { row: usize, detail: String },
}
```

**err-make-illegal-states-unrepresentable** — Newtypes/enums over loose flags.

```rust
// Bad
fn expand(censor: bool, weight: bool) { /* two bools = four states, two invalid */ }

// Good
enum Design { Sequential, CloneCensorWeight } // v1 is Sequential only
```

## 2. Determinism (CRITICAL — see [determinism rule])

**det-no-ambient** — No wall-clock, RNG, threads-affecting-order, or env reads in
the transform.

**det-ordered-collections** — Default to `BTreeMap`/`BTreeSet`, or sort keys
before iterating, whenever iteration order can reach the output.

```rust
// Bad — HashMap iteration order is unspecified -> output rows can reorder
for (k, v) in &map { out.push(row(k, v)); }

// Good — deterministic
for (k, v) in map.iter().collect::<BTreeMap<_, _>>() { out.push(row(k, v)); }
```

**det-total-sort** — Fully specify sort keys, tiebreakers, and null order to match
the Oracle. Never rely on input order.

## 3. Ownership & Idioms (HIGH)

**own-borrow-first** — Take `&str`/`&[T]`; clone deliberately, not reflexively.
Accept `impl AsRef<Path>` / `&str` at API edges.

**idiom-iterators** — Prefer iterator combinators (`map`/`filter`/`try_fold`) over
manual index loops; they're clearer and let LLVM vectorize.

**idiom-from-into** — Implement `From` for conversions; get `Into` and `?`
error-conversion for free.

## 4. Polars Pipelines (HIGH)

**pl-lazy** — Build with `LazyFrame`; `.collect()` once at the end so the optimizer
can fuse projections/filters.

**pl-explicit-schema** — Declare dtypes explicitly on read and on output; never
let inference decide a schema-bearing column. A silent `i32`→`i64` or
`f64`→`f32` is the #1 fixture-mismatch cause.

```rust
let lf = LazyCsvReader::new(path)
    .with_dtype_overrides(Some(schema)) // explicit, not inferred
    .finish()?;
```

**pl-stable-ops** — Use stable sorts with explicit `SortMultipleOptions`
(descending + nulls_last per column). Re-sort to canonical order after any
parallel/group operation.

## 5. Performance (MEDIUM — after correctness)

**perf-measure-first** — Optimize only with a `bench/` number in hand. Correctness
and bit-exactness outrank speed.

**perf-avoid-realloc** — `Vec::with_capacity(n)` when the size is known; reuse
buffers across rows in hot loops.

**perf-borrow-in-hot-paths** — No per-row `String`/`Vec` allocation where a slice
or `Cow` works.

## 6. Dependencies & Safety (MEDIUM)

**dep-justify** — Explain every new dependency; it must pass `cargo deny check`.
Prefer std and crates already in the tree.

**safe-no-unsafe** — `#![forbid(unsafe_code)]` stays. No exceptions.

## 7. Tests & Docs (see [testing rule])

**test-row-diff** — Assertions print the offending rows, so a human can adjudicate
a possible fixture/spec conflict.

**doc-runnable** — Public items carry `///` docs with a runnable example.

## Quick checklist before declaring done

- [ ] `cargo fmt --all --check` clean
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` clean
- [ ] `cargo test` / `cargo nextest run` green, no `#[ignore]`, no weakened asserts
- [ ] No `unwrap`/`expect`/`panic` added to the engine path
- [ ] No new nondeterminism (RNG, clock, hash-order, unstable sort)
- [ ] `cargo deny check` passes if dependencies changed
