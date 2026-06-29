# Determinism (the whole point of this engine)

The engine must reproduce the Oracle **bit-for-bit**. Any nondeterminism is a
correctness bug, not a style nit. Output must be identical across runs, machines,
CPU core counts, locales, and timezones.

## Hard rules
- **No wall-clock time, no RNG, no environment reads** in the transform path. If
  randomness is unavoidable for a test, seed it explicitly and record the seed.
- **No hash-map iteration-order dependence.** Iterating a `HashMap`/`HashSet`
  yields an unspecified order. Use `BTreeMap`/`BTreeSet`, or sort keys before
  iterating, when order affects output.
- **Sorts must be total and explicit.** Specify every tiebreaker column and the
  null ordering so equal keys can't reorder. Match the Oracle's ordering exactly.
- **Floating point is reproduced, not "approximately equal" in the engine.** Do
  the same operations in the same order as the Oracle. Tolerances live in the
  test harness, never in `src/`.
- **Parallelism must not reorder rows.** Polars/Rayon parallelism is fine only
  where the result is order-independent or re-sorted afterward to the canonical
  order.
- **Locale/encoding independence.** No locale-sensitive number/string formatting;
  parse and emit with fixed, explicit formats.

## When output differs from a fixture
Inspect the **row-level diff** first. The usual culprits, in order:
1. A typing mismatch (CSV vs Parquet, int vs float, factor/category ordering).
2. An unstable or under-specified sort.
3. A `SPEC.md`/fixture conflict — the **fixture wins**; STOP and report.

See CLAUDE.md for the iteration loop and the "stop after ~5 non-improving
iterations" rule.
