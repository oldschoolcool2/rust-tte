# oracle/30_edge_cases.R
source("oracle/00_setup.R")

# --------------------------------------------------------------------------
# Eligibility conventions used below (state them so cases don't read as
# contradictory):
#
#  * DEFAULT (E01-E03, E05-E09): eligibility is MONOTONE — eligible == "never
#    yet treated". Once a patient initiates, they are a prevalent user and are
#    ineligible for all later periods, even if they later stop. This matches the
#    package's canonical ID=4 trace (eligible 1,1,1,0,0,... once treated at t=2).
#
#  * E04 ONLY: eligibility is deliberately NON-MONOTONE (eligible -> ineligible
#    -> eligible) to probe re-entry. The gap is a transient exclusion while the
#    patient is still treatment-naive (e.g. a temporary contraindication / out of
#    the risk set), NOT treatment status. This is the one case that breaks the
#    default convention on purpose.
#
# The Oracle output is canonical. Where a comment states an expectation and the
# Oracle disagrees, that is a finding to investigate (per ADR-6, the fixture
# wins) — not a value to paper over.
# --------------------------------------------------------------------------

mk <- function(...) {
  df <- data.frame(..., stringsAsFactors = FALSE)
  for (c in c("eligible","treatment","outcome","x2")) df[[c]] <- as.integer(df[[c]])
  if (is.null(df$x1)) df$x1 <- 0
  df[, INPUT_COLS]
}

EDGE_CASES <- list(

  # E01 — minimal: one eligible patient, one period. Expect: trial_period=0,
  # followup_time=0, single row. Floor case for the whole engine.
  E01_single = mk(id=1, period=0, eligible=1, treatment=0, x1=0, x2=0, outcome=0),

  # E02 — CANONICAL vignette ID=4: eligible t=0,1,2; initiates at t=2; followed to t=9.
  # Expect (ITT): trial 0 assigned=0 fu 0..9; trial 1 assigned=0 fu 0..8;
  #               trial 2 assigned=1 fu 0..7. This is the published reference behaviour.
  E02_id4_canonical = mk(
    id=4, period=0:9,
    eligible  =c(1,1,1,0,0,0,0,0,0,0),
    treatment =c(0,0,1,1,1,1,1,1,1,1),
    x1=0, x2=0, outcome=0
  ),

  # E03 — event ON a trial baseline visit (followup_time=0). Probes whether a
  # baseline-visit outcome is retained vs dropped. Classic off-by-one risk.
  E03_event_at_baseline = mk(id=1, period=0, eligible=1, treatment=0, x1=0, x2=0, outcome=1),

  # E04 — RE-ENTRY: eligible -> eligible -> INELIGIBLE(gap) -> eligible(re-entry) -> ineligible.
  # Probes that a re-entered eligible period seeds a NEW trial_period, with
  # assigned_treatment taken from the RE-ENTRY period (not frozen from first
  # eligibility), and that the ineligible gap seeds NOTHING.
  #   eligible=(1,1,0,1,0), treatment=(0,0,0,1,1): naive through the gap, initiates at t=3.
  # Expect (ITT): three trials — periods 0, 1, 3; NONE at 2 or 4.
  #   trial 0: assigned=0, fu 0..4 (5 rows)
  #   trial 1: assigned=0, fu 0..3 (4 rows)
  #   trial 3: assigned=1, fu 0..1 (2 rows)   <- re-entry; assigned from t=3, distinct from 0/1
  #   => 11 rows, 3 trials.
  # Catches: (a) seeding only first eligibility (=>1 trial); (b) seeding the gap
  # (=>4 trials); (c) freezing baseline assignment (=> trial 3 wrongly assigned=0).
  # Anchor: Fu/Hernán, BMJ 2026;392:e084909 (time-zero alignment, repeated eligibility).
  E04_reentry = mk(
    id=1, period=0:4,
    eligible  =c(1,1,0,1,0),
    treatment =c(0,0,0,1,1),
    x1=0, x2=0, outcome=0
  ),

  # E05 — never initiates: eligible every period, control-only max fan-out.
  # Expect a trial seeded at EVERY period, all assigned=0.
  E05_never_treats = mk(
    id=1, period=0:4, eligible=1, treatment=0, x1=0, x2=0, outcome=0
  ),

  # E06 — SWITCH-THEN-BACK (PP-focused): treatment 1 -> 1 -> 0 -> 1.
  # Patient initiates at baseline (treated arm, assigned=1), adheres one period,
  # DEVIATES (off treatment), then switches back ON. The switch-back is the trap.
  #   eligible=(1,0,0,0): ever-treated => ineligible thereafter (default convention),
  #   so no second trial seeds at t=2 despite treatment=0 there.
  # Expect (ITT): trial 0, assigned=1 frozen, fu 0..3, treatment seen 1,1,0,1, NO
  #               censoring. 4 rows.
  # Expect (PP):  trial 0, assigned=1; adheres fu 0,1; FIRST deviation at fu=2;
  #               artificially censored at first deviation; fu=3 must NOT appear
  #               (switch-back does NOT resume follow-up). Package: "data after
  #               [deviation] are discarded"; expand flag = 1 up to first switch.
  #   The exact row-inclusion at the switch (fu=2 emitted-with-flag vs excluded)
  #   is the Oracle's convention — read it off the dump, do not hard-assert.
  # Catches: row-by-row censoring that wrongly RESUMES the trial at fu=3.
  # GENERATION NOTE: dump this PP fixture via the S4 expand_trials() path
  #   (expansion only, no switch-weight glm) to avoid the degenerate-input glm
  #   crash on a single-patient cohort.
  # Anchor: Danaei 2013; package expand_until_switch / "up until first switch".
  E06_switch_then_back = mk(
    id=1, period=0:3,
    eligible  =c(1,0,0,0),
    treatment =c(1,1,0,1),
    x1=0, x2=0, outcome=0
  ),

  # E07 — eligibility ONLY in the final observed period => single-row trial at the edge.
  E07_last_period_only = mk(
    id=1, period=0:3,
    eligible  =c(0,0,0,1),
    treatment =c(1,1,1,0),  # was treated, then a (contrived) naive final period
    x1=0, x2=0, outcome=0
  ),

  # E08 — TIES: event coincides with end-of-follow-up. Decision: EVENT BEFORE
  # CENSORING. A visit carrying outcome=1 is retained as the event row in every
  # active trial at its respective followup_time, and is the TERMINATING row —
  # never dropped/truncated as if censored.
  #   eligible=(1,1,1), treatment=(0,0,0): three overlapping control trials, never
  #   deviates (so PP == ITT here). outcome=(0,0,1): terminal event at t=2.
  # Expect: one event at period 2 recorded across all three trials —
  #   trial 0: fu 0..2, outcome 0,0,1 (3 rows)
  #   trial 1: fu 0..1, outcome 0,1   (2 rows)
  #   trial 2: fu 0,    outcome 1     (1 row)  <- baseline-visit event (E03 tie, multi-trial)
  #   => 6 rows.
  # NOTE: with the current censor-free INPUT_COLS, the event/censor tie is
  # resolved at input authorship (set outcome=1 when both coincide). A genuine
  # competing censor belongs to a future censor-aware schema; same precedence holds.
  E08_ties = mk(
    id=1, period=0:2,
    eligible  =c(1,1,1),
    treatment =c(0,0,0),
    x1=0, x2=0, outcome=c(0,0,1)
  ),

  # E09 — MAX FAN-OUT: one patient eligible & untreated for periods 0..30 with
  # full follow-up and no event. Stresses the row-count invariant in closed form.
  # Expect (ITT): 31 trials (one per period); trial j has fu 0..(30-j) => (31-j)
  #   rows; total = sum_{j=0}^{30}(31-j) = 31*32/2 = 496 rows; all assigned=0.
  # General invariant: eligible+untreated over periods 0..K, full follow-up, no
  #   event => (K+1)(K+2)/2 expanded rows across (K+1) trials. Also checks:
  #   exactly one fu==0 row per trial (=>31); no fu row precedes its trial_period.
  E09_max_fanout = mk(
    id=1, period=0:30,
    eligible=1, treatment=0, x1=0, x2=0, outcome=0
  )
)
