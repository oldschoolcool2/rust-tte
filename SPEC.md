# SPEC — Behavioural Specification of the Sequential-Trial Expansion

> **Status: §1–§3 + §6 authored for Phase 1 (ITT), from the pinned Oracle
> `TrialEmulation 0.0.4.11`.** R-free: behaviour (pseudocode + types), not R syntax.
>
> **Authority rule:** the Parquet fixtures in `fixtures/` are the final authority.
> **When this SPEC and a fixture disagree, the fixture wins** — stop and report the
> discrepancy with the offending rows.

---

## 1. Glossary & column dictionary

**Input (long person-period) schema** — one row per `(id, period)`, contiguous
periods per `id`:

| Column | Type (edge fixtures) | Type (scenario fixtures) | Meaning |
|---|---|---|---|
| `id` | float64 | int32 | Patient identifier. |
| `period` | int32 (float64 in E01/E03) | int32 | Discrete 0-based time index. |
| `eligible` | int32 `{0,1}` | int32 | `1` ⇒ patient may seed a trial at this period. |
| `treatment` | int32 `{0,1}` | int32 | Observed treatment status at this period. |
| `x1` | float64 | float64 | Time-varying covariate (carried, not matched). |
| `x2` | int32 | int32 | Covariate (carried, not matched). |
| `outcome` | int32 `{0,1}` | int32 | Observed event indicator at this period. |

> The edge fixtures inherit R literal quirks: `mk(id=1)` is an R *double* and
> `period=0` (E01/E03) is a *double*, whereas `period=0:9` (`:`) is an *integer*.
> The engine reads each input parquet **as-is** and never coerces input dtypes.

**Output (expanded) structural columns** — the **bit-exact match set**, in this
order, sorted by `(id, trial_period, followup_time)` ascending:

| Column | Output dtype rule | Meaning |
|---|---|---|
| `id` | **passthrough** of input `id` dtype | Patient identifier. |
| `trial_period` | **Int32** (always) | Period at which this emulated trial starts. |
| `followup_time` | **inherits input `period` dtype** | 0-based offset within the trial (`period − trial_period`). |
| `assigned_treatment` | **passthrough** of input `treatment` dtype | Treatment at trial baseline, carried forward (ITT). |
| `treatment` | **passthrough** of input `treatment` dtype | Actual treatment at `trial_period + followup_time`. |
| `outcome` | **Float64** (always) | Outcome at `trial_period + followup_time`. |

> These dtype rules are not a design choice — they are what `TrialEmulation
> 0.0.4.11`'s `data_preparation()$data` emits (verified per-fixture from the
> generated parquet schemas). The `weight` column it also emits is **not** part of
> the structural match set and is dropped. No per-protocol censor/expand-flag
> column exists in this version (so `STRUCTURAL_COLS` is exactly these six).

## 2. ITT expansion algorithm

Let the input be long person-time, one row per `(id, period)`.

1. **Seeds.** A `(id, period)` row *seeds an emulated trial* iff `eligible == 1`
   and `first_period ≤ period ≤ last_period`. Its `trial_period := period`
   (as Int32) and its `assigned_treatment := treatment` **at that very period**.
   Eligibility is time-varying and evaluated every period: a patient eligible at
   several periods seeds several trials, and `assigned_treatment` is sourced from
   `treatment` at each trial's own `trial_period` — **never** frozen from the first
   eligible period. This is the property that makes re-entry (eligible → ineligible
   → eligible) correct: the re-entered trial takes its assignment from the re-entry
   period (cf. `fixtures/edge/expected_E04_reentry_itt.parquet`, where trials seed
   at periods 0, 1, 3 with `assigned_treatment` 0, 0, **1**). Collapsing repeated
   eligibility to a single time zero is the immortal-time / alignment failure that
   Fu et al. (*BMJ* 2026;392:e084909) warn against.
2. **Follow-up.** For each seed, emit one output row for **every** observed
   `(id, p)` of the same patient with `p ≥ trial_period` (a self-join on `id`
   restricted to `p ≥ trial_period`). On each such row:
   - `followup_time := p − trial_period`,
   - `treatment := ` the patient's *actual* treatment at period `p`,
   - `outcome := ` the patient's outcome at period `p`,
   - `assigned_treatment := ` the seed's baseline treatment, **carried forward
     unchanged** for the whole trial.
3. **No artificial censoring.** ITT does **not** truncate a trial when actual
   treatment deviates from `assigned_treatment`. Follow-up extends to the
   patient's last observed period (the input already ends a patient's rows at
   their first event/censoring, so "until event/censor/end of data" falls out
   naturally — no separate censoring column is consulted).
4. **Order.** Sort the result by `(id, trial_period, followup_time)` ascending.
   This triple is a unique key, so the order is total and deterministic.

`first_period`/`last_period` default to the full observed range (the engine's
defaults `0 … i32::MAX` are a no-op filter for cohorts that start at period 0).

## 3. Per-protocol artificial censoring (`expand_until_switch`) — Phase 2

> Orientation only; **not** implemented in Phase 1. In PP, a trial's follow-up is
> artificially censored at the **first** `followup_time` where actual `treatment`
> deviates from `assigned_treatment`. Confirmed on `TrialEmulation 0.0.4.11`:
> the PP frame carries the **same six** structural columns (no extra flag column);
> censoring manifests purely as *missing* later follow-up rows. Two Oracle caveats
> found while generating fixtures (reported for sign-off, both Oracle-side):
> (a) PP `data_preparation` errors inside `glm` on degenerate single-row inputs
> (e.g. E01); (b) the scenario `validate_input()` uses `data.table` `by=` on a
> plain `data.frame`. Phase 1 therefore ships **ITT-only** fixtures.
>
> First-deviation-only is the package's documented rule — the IPW manuscript
> (arXiv:2402.12083) discards "the data after [the enrollee] started the treatment"
> and emits the expand indicator "up until first switch"; a switch *back* must
> **not** resume follow-up (cf. `E06`, trajectory 1→1→0→1: PP terminates at the
> first deviation, so `followup_time == 3` does not appear). When PP fixtures are
> generated (Phase 2) prefer the S4 `trial_sequence("PP") |> … |> expand_trials()`
> path, which does the structural expansion **without** fitting the switch-weight
> `glm`, sidestepping caveat (a) on degenerate cohorts.

## 4. Weight application — Phase 3

> Join key `(id, trial_period, followup_time)`; multiply the pre-computed IPCW
> column onto the expanded frame. Exact on the join; ~1e-12 only on the float
> product. No solver here — weights come from R.

## 5. Invariants (these also become property tests)

For any valid input, the expanded frame must satisfy:

- **Row count.** Total expanded rows = Σ over each `(id, eligible & in-range
  period)` of `(last_observed_period − trial_period + 1)`.
- **One baseline per trial.** Exactly one `followup_time == 0` row per
  `(id, trial_period)`.
- **No pre-baseline rows.** `followup_time ≥ 0`, and each row maps to
  `period = trial_period + followup_time`.
- **Constant assignment.** `assigned_treatment` is constant within
  `(id, trial_period)`.
- **Assignment source.** `assigned_treatment` for `(id, trial_period)` equals the
  input `treatment` at `period == trial_period` — re-entered trials take assignment
  from the re-entry period, not the first eligible period. (Property test:
  `invariant_assigned_treatment_sourced_from_trial_period`.)
- **(PP, Phase 2) monotone censoring.** Once a PP trial is censored, no later
  `followup_time` rows exist for that `(id, trial_period)`.

## 6. Worked micro-example (E02 — canonical vignette `id = 4`)

Input (`outcome ≡ 0`; eligible at `t = 0,1,2`; initiates treatment at `t = 2`;
observed to `t = 9`):

| period | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| eligible | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| treatment | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |

Three trials are seeded (at `t = 0, 1, 2`), expanding to **27** rows:

- **trial_period 0** (`assigned_treatment = 0`): `followup_time 0…9`, with
  `treatment = 0,0,1,1,1,1,1,1,1,1`.
- **trial_period 1** (`assigned_treatment = 0`): `followup_time 0…8`, with
  `treatment = 0,1,1,1,1,1,1,1,1`.
- **trial_period 2** (`assigned_treatment = 1`): `followup_time 0…7`, with
  `treatment = 1,1,1,1,1,1,1,1`.

`assigned_treatment` stays fixed within each trial; `treatment` tracks the actual
value at `trial_period + followup_time`. Cross-check:
`fixtures/edge/expected_E02_id4_canonical_itt.parquet`.
