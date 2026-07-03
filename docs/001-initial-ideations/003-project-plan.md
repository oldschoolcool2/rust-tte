# tte-expand — Project Plan & Agentic Build Roadmap

**A verified, high-performance Rust backend for the data-expansion stage of sequential target trial emulation.**

> Working names (pick later): Rust crate `tte-expand` · R companion package `tters` (extendr binding) · repo `tte-expand`.

---

## 0. Ground truth (verified facts this plan is built on)

| Item | Value |
|---|---|
| Upstream package | `TrialEmulation` (CRAN + GitHub) |
| Repo | `github.com/Causal-LDA/TrialEmulation` |
| Maintainer | Isaac Gravestock (Roche); authors Li Su (MRC-BSU), Roonak Rezvani (original), Julia Moesch (Roche); funded by MRC + Roche |
| License | **Apache-2.0** (permissive — derived works fine if license preserved) |
| Design implemented | **Sequential trial emulation** (Hernán 2008 / Gran 2010 / Danaei 2013) — *not* grace-period clone-censor-weight |
| Hot path | `data_preparation()` / `expand_trials()` on `trial_sequence_*` objects |
| Existing perf workarounds | `chunk_size`, `separate_files` (because expanded data does not fit in RAM); censoring already offloaded to C++ (`censor_func`) |
| Statistical deps to NOT reimplement | `parglm` (pooled logistic for weights), `sandwich` (robust variance) |
| Seed example data | `data_censored` (89 patients, ≤19 periods); also `trial_example`, `te_data_ex` |

**The one correction that reshapes everything:** the package emulates a *sequence of trials*, where each eligible period spawns a trial with columns `trial_period`, `followup_time`, `assigned_treatment`, `treatment`, `outcome`. It does **not** clone each patient into Treatment/Control arms with a 90-day grace period (that is the separate CCW design). All prompts and fixtures below target the sequential expansion. If you ever want the CCW grace-period design, that is a *different, standalone crate* — do not mix the two in one validation loop.

---

## 1. Thesis & contribution framing

**What this is:** a memory-safe, out-of-core Rust engine that reproduces the sequential-trial expansion *bit-for-bit*, exposed back to R users as a drop-in faster `data_preparation` backend.

**What this is NOT:** a rewrite of the package, a new statistical method, or a reimplementation of `glm`/`sandwich`.

**Why it is a real contribution (not just "faster"):**
1. The expansion is the documented scaling wall of the gold-standard tool — the maintainers built file-chunking to cope with it.
2. The deliverable that gives it scientific weight is the **computational-reproducibility certificate**: a public proof of bit-exact equivalence to the CRAN reference across an adversarial fixture battery. That artifact speaks directly to the RWE-reproducibility conversation (FDA/EMA/ENCePP) — a verified reimplementation is more citable than a benchmark.
3. Apache-2.0 + active maintainers + "manuscript in preparation" = a timely, welcome contribution rather than an unsolicited fork.

---

## 2. Scope

| In scope (v1) | Out of scope (v1 — keep in R) |
|---|---|
| Sequential expansion: long input → expanded trial frame | Pooled logistic weight *fitting* (`parglm`) |
| ITT expansion (carry assigned treatment forward) | Robust / sandwich variance (`sandwich`) |
| Per-protocol artificial censoring (`expand_until_switch`) | MSM coefficient estimation, CIs |
| Weight *application* (multiply pre-computed weights) | Any novel methodology |
| extendr binding + R companion package | Bayesian/MCMC anything |
| Reproducibility validation suite + benchmark | The CCW grace-period design |

**Rule of thumb:** Rust owns *deterministic data transformation*. R keeps *statistical estimation* until v2+, and even then you bind a mature solver rather than hand-rolling one.

---

## 3. Architecture decisions

- **ADR-1 — R is the Oracle.** The forked R package run on fixed seed data produces immutable expected outputs. The Rust code's only job is to match them. The Oracle is ground truth; never "fix" the Oracle to make Rust pass.
- **ADR-2 — Tolerance is staged by stage.**
  - Expansion / censoring flags → **exact** (integer + categorical; a diff is a bug).
  - Weight application (given pre-computed weights) → exact on the join, tolerance only on the float product (~1e-12).
  - Anything involving a solver (v2+) → a *scientifically justified* tolerance (e.g. ~1e-6 on weights, ~1e-4 on log-HR), documented and locked in the harness. You will never match R's IRLS bit-for-bit; do not try.
- **ADR-3 — Fixtures are Parquet, never CSV.** CSV silently coerces int/categorical/NA typing and round-trips floats — manufacturing false mismatches that burn agent loops. Parquet preserves dtypes.
- **ADR-4 — Polars (lazy) engine, `#![forbid(unsafe_code)]`.** Out-of-core via lazy/streaming to beat the RAM wall that forced upstream chunking.
- **ADR-5 — extendr is the bridge, R is the first target.** Both the userbase and the Oracle live in R, so ship the R binding first. PyO3 for the Python RWE-pipeline crowd is a fast-follow, not v1.
- **ADR-6 — Feed the agent a behavioral spec, not R source.** Pasting R source pushes the model into line-by-line translation mode (the "ecosystem trap"). Give it `SPEC.md` (plain pseudocode/math) + the Parquet fixtures. **When SPEC.md and a fixture disagree, the fixture wins and the agent must flag the discrepancy, not silently follow either.**

---

## 4. Repository layout

```
tte-expand/
├── CLAUDE.md                  # persistent agent rules (Section 8) — read every session
├── SPEC.md                    # behavioral spec of the expansion (Section 10)
├── ROADMAP.md                 # this file (or a link to it)
├── oracle/                    # FORKED R package + fixture generation — agent NEVER edits
│   ├── renv.lock              # pinned R + package versions for reproducible fixtures
│   ├── generate_fixtures.R    # runs upstream on seed data, dumps input+expected to Parquet
│   └── TrialEmulation/        # git submodule or vendored fork of Causal-LDA/TrialEmulation
├── fixtures/                  # GENERATED Parquet files — agent NEVER edits, only reads
│   ├── itt/   pp/   weights/  # one subdir per phase
│   └── MANIFEST.json          # sha256 of each fixture + which Oracle version produced it
├── crates/
│   └── tte-expand/
│       ├── src/               # ONLY directory the agent may modify
│       └── tests/             # cargo tests — agent NEVER edits (Section 7)
├── bindings/
│   └── tters/                 # extendr R package (Phase 4)
├── bench/                     # criterion benchmarks vs R (Phase 5)
└── report/                    # reproducibility certificate + benchmark writeup (Phase 5)
```

Hard filesystem boundary the agent must respect: **writable = `crates/tte-expand/src/` only.** Everything else is read-only contract.

---

## 5. The Oracle & fixture strategy

**Generation.** `oracle/generate_fixtures.R` loads each seed dataset, runs the upstream expansion (`trial_sequence_ITT` → `set_data` → `set_expansion_options` → `expand_trials`, or the legacy `data_preparation`), and writes `input_<case>.parquet` + `expected_<case>.parquet` via `arrow::write_parquet`. Record the upstream version + a sha256 of every file in `MANIFEST.json`.

**Reproducibility.** Pin R and all package versions with `renv`. Pin the Rust toolchain with `rust-toolchain.toml`. CI regenerates fixtures from the pinned Oracle and fails if any sha256 drifts — so the contract can never silently change underneath you.

**The fixture battery is the moat — and it is an epidemiology task, not a Rust task.** A happy-path fixture passes while the logic is subtly wrong. Build adversarial cases that probe exactly where sequential expansion mishandles immortal time / eligibility:

1. Patient eligible at **multiple** `trial_period`s (the core behavior) vs eligible only at baseline.
2. Event or censoring **on the trial baseline visit** (`followup_time = 0`).
3. Treatment switch **exactly at a trial boundary** (stresses `expand_until_switch`).
4. **ITT vs PP divergence** on the same patient (PP artificially censors at first deviation; ITT does not).
5. **Last-period eligibility** → single-row trials.
6. **Ties** in event/censor timing.
7. A patient who **never initiates** treatment.
8. A patient eligible, then **ineligible, then eligible again**.
9. Minimal fixtures: 1 patient / 1 period; 1 patient eligible every period (max fan-out).

Start each phase's tests on case 1 (simplest) and add cases in roughly the order above so the agent gets a graded difficulty ramp, not a single cliff.

---

## 6. Roadmap (phases with Definition of Done)

**Phase 0 — Scaffold (you, not the agent).** Fork upstream into `oracle/`; write `generate_fixtures.R`; pin R/Rust; write `SPEC.md`; write the empty Rust crate with failing tests that load fixtures and assert against an `unimplemented!()` function. *DoD:* `cargo test` runs and fails cleanly with a clear "not implemented" message; fixtures exist with a manifest.

**Phase 1 — ITT expansion, exact match.** Agent implements `expand_itt()`. *DoD:* bit-exact match on all ITT fixtures (cases 1–9) for `id, trial_period, followup_time, assigned_treatment, treatment, outcome`; all property tests pass; `forbid(unsafe_code)` holds.

**Phase 2 — Per-protocol artificial censoring.** Agent adds the `expand_until_switch` deviation logic. *DoD:* exact match on PP fixtures incl. the ITT-vs-PP divergence case; ITT path unchanged (no regressions).

**Phase 3 — Weight application.** Given a column of pre-computed IPCW (from the R Oracle), Rust joins/applies them onto the expanded frame. *DoD:* exact join, float product within 1e-12. (No solver — weights come from R.)

**Phase 4 — extendr binding.** Wrap the engine as R-callable; the companion package exposes a `data_preparation`-compatible entry point. *DoD:* R round-trip (`R data.frame → Rust → R data.frame`) matches upstream output on the full battery; installs cleanly via `R CMD INSTALL`.

**Phase 5 — Benchmark + reproducibility certificate.** Criterion benchmarks vs upstream across N = 10³…10⁷ rows and memory ceilings that OOM the R path; generate the validation report. *DoD:* report shows bit-exact equivalence + speed/memory curves; runs end-to-end in CI.

**Phase 6 (optional, v2) — Weights in Rust.** Only if justified. Bind a mature logistic solver; keep robust/sandwich variance in R. *DoD:* weights within documented tolerance of `parglm` output; explicit statement of where and why exactness ends.

---

## 7. Agentic loop protocol

**Inner loop (every iteration):**
1. Read the single failing test and the relevant `SPEC.md` section.
2. Modify only `crates/tte-expand/src/`.
3. Run `cargo test <specific_test>` (not the whole suite — keep signal tight).
4. If red: read the *actual row-level diff*, revise, repeat.
5. If green: run the full suite to check for regressions; move to next case.
6. Stop when the whole phase's suite is green.

**Guardrails (non-negotiable — also in CLAUDE.md):**
- Never edit `tests/`, `fixtures/`, `oracle/`, or `SPEC.md`. Tolerances live in the harness; the agent cannot relax them.
- Never add `#[ignore]`, never weaken an assertion, never special-case a fixture by hard-coding its expected value.
- Never invent statistics. If matching requires a solver, **stop and report** — do not hand-roll one in v1.
- If `SPEC.md` contradicts a fixture, stop and report the discrepancy with the offending rows.
- No `unsafe`. No new dependencies without flagging why.

**Context-window / worktree management (you):**
- One git worktree per phase; keep the agent's context scoped to `src/` + current fixtures + the relevant `SPEC.md` section. Do **not** load the R source or the whole repo.
- Fixtures are the contract; the methods description is orientation. Keep both small.
- Use a fresh session per phase to avoid drift; carry forward only `CLAUDE.md` + the phase prompt.

**Escalation / runaway detection:** if the agent loops > ~5 times on the same test without reducing the diff, halt. Causes are almost always (a) a typing mismatch (CSV vs Parquet, int vs float, factor ordering), (b) a `SPEC.md`/fixture conflict, or (c) genuinely needing a capability that's out of scope. All three are *your* decision, not the agent's.

---

## 8. `CLAUDE.md` (drop into repo root)

```markdown
# tte-expand — Agent Operating Rules

## What you are building
A Rust+Polars engine that reproduces, BIT-FOR-BIT, the sequential
trial-emulation data expansion from the R package `TrialEmulation`.
You are matching a fixed Oracle output. You are NOT doing epidemiology
and NOT inventing statistics — you are satisfying a rigid data schema.

## The contract
- Ground truth = the Parquet fixtures in `fixtures/`. They are immutable.
- Orientation = `SPEC.md`. If SPEC.md and a fixture disagree, the
  FIXTURE WINS — stop and report the discrepancy. Do not silently follow
  either one.
- Tolerances are defined in the test harness, not by you.

## You MAY edit
- `crates/tte-expand/src/` only.

## You MUST NEVER
- Edit `tests/`, `fixtures/`, `oracle/`, or `SPEC.md`.
- Add `#[ignore]`, weaken/skip an assertion, or hard-code a fixture's
  expected values to force a pass.
- Reimplement glm / logistic regression / sandwich variance. If a test
  appears to need a statistical solver, STOP and report — that is out of
  scope for this phase.
- Add `unsafe`. Add dependencies without first explaining why.

## Workflow each iteration
1. Read the ONE failing test + its `SPEC.md` section.
2. Edit `src/`. Run `cargo test <that_test>`.
3. On failure, inspect the actual row-level diff and revise.
4. On success, run the full suite to check for regressions.
5. Stop when the phase suite is fully green. Report what changed and why.

## When stuck
After ~5 non-improving iterations, STOP and report: the failing test,
the smallest reproducing diff, and your hypothesis (typing mismatch?
spec/fixture conflict? out-of-scope capability?). Do not thrash.

## Definition of done (current phase is set in the task prompt)
Bit-exact match on the named columns across all named fixtures, all
property tests green, no regressions, no unsafe.
```

---

## 9. Loop prompts (copy-paste)

### 9a. Reusable task-prompt template

```
PHASE: <n> — <name>
GOAL: Implement `<fn_name>()` in crates/tte-expand/src/ so the tests in
tests/<file>.rs pass.

CONTRACT
- Expected outputs are fixtures/<subdir>/expected_*.parquet. Inputs are
  fixtures/<subdir>/input_*.parquet. These are immutable ground truth.
- Match these columns EXACTLY (no tolerance): <columns>.
- SPEC.md §<x> describes the algorithm for orientation. If it conflicts
  with a fixture, STOP and report — the fixture wins.

RULES
- Edit only crates/tte-expand/src/. Never touch tests/, fixtures/,
  oracle/, SPEC.md. No unsafe. No new deps without asking.
- Do not weaken tests or hard-code expected values.
- Do not implement any statistical solver. If you think you need one,
  STOP and report.

PROCEDURE
- Make the SIMPLEST fixture pass first, then add cases in the order they
  appear in the test file. Run `cargo test` after each change. Inspect
  the row-level diff on failure. Stop when the whole file is green, then
  run the full suite to confirm no regressions.

DONE WHEN
- All tests in tests/<file>.rs pass, all property tests pass, full suite
  green, no unsafe. Then summarize exactly what you changed and why.
```

### 9b. Phase 1 — concrete

```
PHASE: 1 — ITT sequential expansion
GOAL: Implement `expand_itt(input: LazyFrame) -> LazyFrame` in
crates/tte-expand/src/ so tests/itt.rs passes.

WHAT THE FUNCTION DOES (orientation — SPEC.md §2 is authoritative,
fixtures override both):
- Input is long format: one row per (id, period) with columns
  id, period, treatment, outcome, eligible, and covariates.
- For each id, for each `period` where eligible == 1, emit an emulated
  trial with trial_period = that period.
- Within a trial, emit follow-up rows followup_time = 0,1,2,... over the
  patient's subsequent observed periods until their event/censor/end.
- assigned_treatment = the patient's `treatment` at the trial's baseline
  period, carried forward unchanged for all follow-up rows (ITT).
- `treatment` on each follow-up row = the patient's actual treatment at
  (trial_period + followup_time). `outcome` = outcome at that period.
- ITT does NOT artificially censor on treatment switching.

CONTRACT
- Inputs: fixtures/itt/input_*.parquet
- Expected: fixtures/itt/expected_*.parquet
- Match EXACTLY: id, trial_period, followup_time, assigned_treatment,
  treatment, outcome (and exact row count + row order per expected).

RULES / PROCEDURE / DONE: as in the template (§9a). Start with
input_single_patient_single_period, end with input_eligible_every_period.
```

### 9c. Phase 2 — concrete delta

```
PHASE: 2 — Per-protocol artificial censoring
GOAL: Add PP support so tests/pp.rs passes WITHOUT breaking tests/itt.rs.
DELTA FROM PHASE 1:
- In PP, a trial's follow-up is artificially censored at the first
  followup_time where actual `treatment` deviates from
  assigned_treatment (see SPEC.md §3, expand_until_switch semantics).
- Emit the censoring indicator column exactly as in expected_*.parquet.
- The ITT code path and all Phase 1 fixtures must remain bit-exact.
CONTRACT: match EXACTLY id, trial_period, followup_time,
assigned_treatment, treatment, outcome, <censor_flag_col>.
RULES / PROCEDURE / DONE: as template. The ITT-vs-PP divergence fixture
is the key test — make the simpler PP cases pass first.
```

---

## 10. `SPEC.md` contents (you author this)

Keep it plain and R-free. Sections:
1. **Glossary & column dictionary** — every input/output column, type, meaning.
2. **ITT expansion algorithm** — numbered pseudocode (mirror §9b), no R.
3. **PP artificial censoring** — the deviation rule and `expand_until_switch` semantics.
4. **Weight application** — join key and the multiplication.
5. **Invariants** (these also become property tests):
   - total expanded rows = Σ over (id, eligible period) of that trial's follow-up length;
   - exactly one `followup_time == 0` row per `(id, trial_period)`;
   - no `followup_time` row precedes its `trial_period`;
   - `assigned_treatment` is constant within `(id, trial_period)`.
6. **Worked micro-example** — the `id = 4`-style trace from the upstream vignette, expanded by hand, as a sanity anchor.

> Derive §2–§3 from the upstream *Getting Started* vignette and the `expand`/`expand_until_switch` docs — but encode behavior, not R syntax. The fixtures remain the final authority.

---

## 11. Validation & reproducibility deliverables (the scientific artifact)

- **Equivalence certificate:** a generated report asserting bit-exact match on every fixture, with the Oracle version, fixture sha256s, and toolchain pins — reproducible from `make verify`.
- **Property-based coverage:** `proptest` over randomized valid inputs proving the §10 invariants hold beyond the fixed fixtures.
- **Benchmark curves:** runtime + peak RSS vs upstream across row counts, including a regime where the R path OOMs and Rust does not.
- **Differential CI:** fixtures regenerated from the pinned Oracle on every run; build fails on any drift.

This bundle — not the speedup alone — is what makes it citable and regulatory-relevant.

---

## 12. Risks & failure modes

| Risk | Mitigation |
|---|---|
| Agent reimplements `glm`/`sandwich` and flails | Scope rule + CLAUDE.md "STOP and report"; weights stay in R for v1 |
| CSV typing creates false mismatches | Parquet only (ADR-3) |
| Agent games the tests | Read-only `tests/`; tolerances in harness; no `#[ignore]` rule |
| Chasing R `glm` bit-parity | Staged tolerance (ADR-2); exactness only where it's deterministic |
| Conflating sequential TE with CCW grace period | This plan targets sequential expansion only; CCW is a separate crate |
| Oracle drift between R versions | `renv.lock` + sha256 manifest + CI regeneration |
| Floating-point order-of-operations diffs | Keep v1 to integer/categorical exactness; defer float-heavy stages |
| Upstream changes API mid-project | Pin to a specific upstream commit; bump deliberately |

---

## 13. Contribution pathway

1. **Contact the maintainers.** Open a GitHub issue on `Causal-LDA/TrialEmulation` proposing an optional Rust expansion backend (they note a methods manuscript is in prep). Maintainer: Isaac Gravestock (Roche); methods lead: Li Su (MRC-BSU).
2. **Ship as a companion first**, not a fork: `tters` (extendr) calling `tte-expand`, with a `data_preparation`-compatible entry point. Lower friction than an upstream PR; can be upstreamed later.
3. **License:** Apache-2.0 to match upstream; preserve their NOTICE. Confirm before vendoring any fixtures derived from their example data (example data is shipped with the package; deriving fixtures by running it is clean, but cite it).
4. **Write it up:** a JOSS software paper for the crate + a short repro/methods note ("bit-exact reproduction + N× speed/memory") suitable for a pharmacoepi or comp-stats venue. Tie it to the RWE computational-reproducibility theme (ENCePP-relevant).
5. **Framing:** a verified high-performance backend for the gold-standard sequential TTE tool, developed as a companion to the upstream package rather than a replacement for it.

---

## 14. First-week checklist

- [ ] Fork upstream into `oracle/` (pin a specific commit).
- [ ] `renv` + `rust-toolchain.toml` pins; `MANIFEST.json` scheme.
- [ ] `generate_fixtures.R`: dump `data_censored` ITT input + expected to Parquet (start with cases 1–3).
- [ ] Write `SPEC.md` §1–§2 + the worked micro-example.
- [ ] Scaffold crate: failing `tests/itt.rs` loading fixtures, `expand_itt()` = `unimplemented!()`.
- [ ] Drop in `CLAUDE.md`; confirm `cargo test` fails cleanly.
- [ ] Run the Phase 1 prompt (§9b) in a scoped worktree.
- [ ] Open the upstream issue introducing the idea.
```
