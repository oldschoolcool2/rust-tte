# SPEC — Behavioural Specification of the Sequential-Trial Expansion

> **Status: DRAFT / scaffold.** This document is *orientation* for the engine, to
> be authored before Phase 1 begins. It is deliberately **R-free**: it describes
> behaviour (pseudocode + math), not R syntax.
>
> **Authority rule:** the Parquet fixtures in `fixtures/` are the final authority.
> **When this SPEC and a fixture disagree, the fixture wins** — stop and report the
> discrepancy with the offending rows. Derive §2–§3 from the upstream *Getting
> Started* vignette and the `expand` / `expand_until_switch` docs, but encode
> behaviour, not R.

---

## 1. Glossary & column dictionary

> _TODO: freeze every input/output column, its type, and meaning from
> `names(prep$data)` on the pinned Oracle version (see
> `docs/001-initial-ideations/004-prework-fixtures.md` → "VERIFY FIRST")._

**Input (long person-period) schema** — one row per `(id, period)`:

| Column | Type | Meaning |
|---|---|---|
| `id` | int | Patient identifier. |
| `period` | int | Discrete time index (0-based), contiguous per `id`. |
| `eligible` | int {0,1} | 1 ⇒ patient may seed a new trial at this period. |
| `treatment` | int {0,1} | Observed treatment status at this period. |
| `outcome` | int {0,1} | Observed event indicator at this period. |
| `x1`, `x2`, … | covariates | Carried through; not part of the structural match. |

**Output (expanded) structural columns** — the **bit-exact match set**:

| Column | Type | Meaning |
|---|---|---|
| `id` | int | Patient identifier. |
| `trial_period` | int | The period at which this emulated trial starts. |
| `followup_time` | int | 0-based offset within the trial (0 = trial baseline). |
| `assigned_treatment` | int {0,1} | Treatment at the trial baseline, carried forward (ITT). |
| `treatment` | int {0,1} | Actual treatment at `trial_period + followup_time`. |
| `outcome` | int {0,1} | Outcome at `trial_period + followup_time`. |
| _PP censoring flag_ | int {0,1} | _TODO: confirm exact name on the pinned version._ |

## 2. ITT expansion algorithm

> _TODO: numbered pseudocode. Sketch (orientation only — fixtures override):_

1. Input is long format: one row per `(id, period)`.
2. For each `id`, for each `period` where `eligible == 1`, emit an emulated trial
   with `trial_period = period`.
3. Within a trial, emit follow-up rows `followup_time = 0, 1, 2, …` over the
   patient's subsequent observed periods until event / censor / end of follow-up.
4. `assigned_treatment` = the patient's `treatment` at the trial's baseline
   period, carried forward unchanged for all follow-up rows (ITT).
5. `treatment` on each follow-up row = the patient's actual treatment at
   `(trial_period + followup_time)`; `outcome` = outcome at that period.
6. ITT does **not** artificially censor on treatment switching.

## 3. Per-protocol artificial censoring (`expand_until_switch`)

> _TODO: the deviation rule. In PP, a trial's follow-up is artificially censored
> at the first `followup_time` where actual `treatment` deviates from
> `assigned_treatment`. Confirm first-deviation-only semantics and the exact
> censoring-flag column from the fixtures._

## 4. Weight application

> _TODO: the join key (`id, trial_period, followup_time`) and the multiplication
> of the pre-computed IPCW column onto the expanded frame. Exact on the join;
> tolerance ~1e-12 only on the float product. No solver here — weights come from R._

## 5. Invariants (these also become property tests)

For any valid input, the expanded frame must satisfy:

- **Row count.** Total expanded rows = Σ over each `(id, eligible period)` of that
  trial's follow-up length.
- **One baseline per trial.** Exactly one `followup_time == 0` row per
  `(id, trial_period)`.
- **No pre-baseline rows.** No `followup_time` row precedes its `trial_period`
  (equivalently, `followup_time >= 0` and maps to `period = trial_period + followup_time`).
- **Constant assignment.** `assigned_treatment` is constant within
  `(id, trial_period)`.
- **(PP) monotone censoring.** Once a PP trial is censored, no later
  `followup_time` rows exist for that `(id, trial_period)`.

## 6. Worked micro-example

> _TODO: the `id = 4`-style trace from the upstream vignette, expanded by hand, as
> a sanity anchor. (Eligible at `t = 0,1,2`; initiates at `t = 2`; followed to
> `t = 9` ⇒ trial 0 assigned=0 fu 0..9; trial 1 assigned=0 fu 0..8; trial 2
> assigned=1 fu 0..7.) Cross-check against `fixtures/edge/expected_E02_*`._
