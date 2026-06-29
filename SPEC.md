# SPEC — Behavioural Specification of the Sequential-Trial Expansion

> **Status: §1–§3 + §5–§6 authored from the pinned Oracle
> `TrialEmulation 0.0.4.11`. §2 (ITT) shipped in Phase 1; §3 (per-protocol)
> finalised and shipped in Phase 2.** R-free: behaviour (pseudocode + types),
> not R syntax.
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
order, sorted by `(id, trial_period, followup_time)` ascending. **Both** the ITT
(§2) and per-protocol (§3) estimands emit exactly these six columns with these
dtypes; PP differs from ITT only by having *fewer rows* (see §3):

| Column | Output dtype rule | Meaning |
|---|---|---|
| `id` | **passthrough** of input `id` dtype | Patient identifier. |
| `trial_period` | **Int32** (always) | Period at which this emulated trial starts. |
| `followup_time` | **inherits input `period` dtype** | 0-based offset within the trial (`period − trial_period`). |
| `assigned_treatment` | **passthrough** of input `treatment` dtype | Treatment at trial baseline, carried forward. |
| `treatment` | **passthrough** of input `treatment` dtype | Actual treatment at `trial_period + followup_time`. |
| `outcome` | **Float64** (always) | Outcome at `trial_period + followup_time`. |

> These dtype rules are not a design choice — they are what `TrialEmulation
> 0.0.4.11` emits (verified per-fixture from the generated parquet schemas). The
> `weight` column it also emits is **not** part of the structural match set and is
> dropped. No per-protocol censor/expand-flag column exists in this version, so
> `STRUCTURAL_COLS` is exactly these six for **both** estimands.

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

## 3. Per-protocol artificial censoring (`expand_until_switch`)

Per-protocol (PP) takes **each ITT-emulated trial** (§2) and artificially censors
its follow-up at the **first** `followup_time` where the actual `treatment`
deviates from that trial's `assigned_treatment`.

1. **First-deviation only, deviating row EXCLUDED.** Within each
   `(id, trial_period)`, ordered by `followup_time`, find the first row where
   `treatment ≠ assigned_treatment` ("the deviation"). Keep the rows **strictly
   before** it and drop the deviation row itself and everything after it.
2. **Switch-back never resumes.** Once a trial deviates it is censored for good;
   if the patient later returns to the assigned treatment, that row is **not**
   re-introduced (it lies after the first deviation). `E06` (trajectory 1→1→0→1,
   `assigned=1`) is the canonical trap: PP terminates at the first deviation, so
   `followup_time == 3` does **not** appear.
3. **Baseline is always kept.** The baseline row (`followup_time == 0`) never
   deviates by construction — `assigned_treatment` *is* the treatment at the
   trial's own baseline period — so the first deviation is always at
   `followup_time ≥ 1` and every emulated trial retains at least its baseline row.
4. **Operational rule (what the engine computes).** Within each
   `(id, trial_period)` window ordered by `followup_time`, let
   `deviated := (treatment ≠ assigned_treatment) ∈ {0,1}`. Keep a row iff the
   **cumulative maximum** of `deviated` up to and including it is `0` — i.e. the
   adherent prefix. The cumulative flips to `1` at the first deviation and stays
   `1`, which discards the deviation row and every later row in one step.
5. **Order.** Unchanged from §2: `(id, trial_period, followup_time)` ascending.

### 3.1 Schema & the `assigned_treatment == treatment` identity

PP emits the **same six structural columns and dtypes** as ITT (§1). Censoring
manifests **purely as missing follow-up rows** — there is no extra censor/expand
flag column, and no `weight` column. On **every** retained PP row,
`treatment == assigned_treatment` (the kept rows are exactly the adherent prefix),
so `assigned_treatment` is constant within a trial and coincides with `treatment`
wherever a row survives. This holds across all 17 fixtures.

### 3.2 Generation provenance (`PP = ITT ∩ S4-survivors`)

The PP fixtures are generated by intersecting the ITT frame with the rows the
modern S4 expansion retains, **not** via `data_preparation(estimand_type = "PP")`:

- The legacy `data_preparation` PP path fits a switch-weight `glm` that errors
  (`Argument mu must be a nonempty numeric vector`) on degenerate single-patient /
  control-only cohorts (E01, E03, E05, E07, E08, E09). It crashes **exactly** on
  the cohorts where PP == ITT.
- The censoring is therefore taken from the modern S4 path
  `trial_sequence("PP") |> set_data(…) |> set_expansion_options(output =`
  `save_to_datatable(), chunk_size = 0) |> expand_trials()`, which performs the
  structural expansion **without** fitting any `glm` and never crashes, and is
  then projected onto the dtype-exact ITT frame. Wherever the legacy PP path does
  run (E02, E04, E06 + all 8 scenarios) the two agree on the retained row-set
  bit-for-bit.
- The S4 datastore's native schema is
  `id, trial_period, followup_time, outcome, weight, treatment`; the placeholder
  `weight` (all `1.0`, no weight model is fitted) is dropped and
  `assigned_treatment` reattached from the ITT frame — exactly as the ITT path
  drops its own `weight`.

### 3.3 Divergence map

PP ≠ ITT on **E02, E04, E06** and **every** simulated scenario. PP == ITT on the
control-only / single-row cases **E01, E03, E05, E07, E08, E09** (they never
deviate); their PP fixtures are produced and matched all the same.

### 3.4 Worked micro-examples

**E06 — switch-back trap.** Input `treatment = 1,1,0,1`; eligible only at `t = 0`
(ever-treated ⇒ ineligible after), so a single trial at `trial_period 0` with
`assigned_treatment = 1`:

| followup_time | 0 | 1 | 2 | 3 |
|---|---|---|---|---|
| treatment | 1 | 1 | **0** | 1 |
| ITT | keep | keep | keep | keep |
| PP | keep | keep | *drop (first deviation)* | *drop (after deviation)* |

ITT = 4 rows; PP = 2 rows (`followup_time` 0, 1). The switch-back at
`followup_time 3` is **gone**. Cross-check
`fixtures/edge/expected_E06_switch_then_back_pp.parquet`.

**E02 — ITT-vs-PP divergence** (canonical `id = 4`; eligible `t = 0,1,2`;
`treatment = 0,0,1,1,1,1,1,1,1,1`). Three trials:

- `trial_period 0` (`assigned = 0`): treatment `0,0,1,…` → first deviation at
  `followup_time 2` → PP keeps `followup_time 0,1` (2 rows; ITT had 10).
- `trial_period 1` (`assigned = 0`): treatment `0,1,…` → first deviation at
  `followup_time 1` → PP keeps `followup_time 0` (1 row; ITT had 9).
- `trial_period 2` (`assigned = 1`): treatment `1,1,…` → never deviates → PP keeps
  `followup_time 0…7` (8 rows).

Total: **27 ITT rows → 11 PP rows.** Cross-check
`fixtures/edge/expected_E02_id4_canonical_pp.parquet`.

**E04 — re-entry.** Input `eligible = 1,1,0,1,0`, `treatment = 0,0,0,1,1`; trials
at `0, 1, 3`:

- `trial_period 0` (`assigned = 0`): treatment `0,0,0,1,1` → first deviation at
  `followup_time 3` → PP keeps `followup_time 0,1,2` (3 rows; ITT had 5).
- `trial_period 1` (`assigned = 0`): treatment `0,0,1,1` → first deviation at
  `followup_time 2` → PP keeps `followup_time 0,1` (2 rows; ITT had 4).
- `trial_period 3` (`assigned = 1`): treatment `1,1` → never deviates → PP keeps
  `followup_time 0,1` (2 rows).

Total: **11 ITT rows → 7 PP rows.** Cross-check
`fixtures/edge/expected_E04_reentry_pp.parquet`.

## 4. Weight application — Phase 3

> Join key `(id, trial_period, followup_time)`; multiply the pre-computed IPCW
> column onto the expanded frame. Exact on the join; ~1e-12 only on the float
> product. No solver here — weights come from R.

## 5. Invariants (these also become property tests)

For any valid input, the expanded frame must satisfy:

- **Row count (ITT).** Total expanded rows = Σ over each `(id, eligible & in-range
  period)` of `(last_observed_period − trial_period + 1)`.
- **One baseline per trial.** Exactly one `followup_time == 0` row per
  `(id, trial_period)` (both estimands).
- **No pre-baseline rows.** `followup_time ≥ 0`, and each row maps to
  `period = trial_period + followup_time`.
- **Constant assignment.** `assigned_treatment` is constant within
  `(id, trial_period)`.
- **Assignment source.** `assigned_treatment` for `(id, trial_period)` equals the
  input `treatment` at `period == trial_period` — re-entered trials take assignment
  from the re-entry period, not the first eligible period. (Property test:
  `invariant_assigned_treatment_sourced_from_trial_period`.)
- **(PP) Monotone censoring & adherence.** Within each `(id, trial_period)` the
  retained PP follow-up is a **contiguous prefix** `followup_time = 0,1,…,k` with
  **no** row at or after the first deviation; **every** retained row is adherent
  (`treatment == assigned_treatment`); and a switch-back never re-introduces a
  later row. PP rows are a subset of the ITT rows for the same input. (Property
  test: `invariant_pp_monotone_censoring`.)

## 6. Worked micro-example (E02 — canonical vignette `id = 4`)

Input (`outcome ≡ 0`; eligible at `t = 0,1,2`; initiates treatment at `t = 2`;
observed to `t = 9`):

| period | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 |
|---|---|---|---|---|---|---|---|---|---|---|
| eligible | 1 | 1 | 1 | 0 | 0 | 0 | 0 | 0 | 0 | 0 |
| treatment | 0 | 0 | 1 | 1 | 1 | 1 | 1 | 1 | 1 | 1 |

Three trials are seeded (at `t = 0, 1, 2`), expanding to **27** ITT rows:

- **trial_period 0** (`assigned_treatment = 0`): `followup_time 0…9`, with
  `treatment = 0,0,1,1,1,1,1,1,1,1`.
- **trial_period 1** (`assigned_treatment = 0`): `followup_time 0…8`, with
  `treatment = 0,1,1,1,1,1,1,1,1`.
- **trial_period 2** (`assigned_treatment = 1`): `followup_time 0…7`, with
  `treatment = 1,1,1,1,1,1,1,1`.

`assigned_treatment` stays fixed within each trial; `treatment` tracks the actual
value at `trial_period + followup_time`. Cross-check
`fixtures/edge/expected_E02_id4_canonical_itt.parquet`. The per-protocol view of
the same cohort (11 rows) is worked through in §3.4.
