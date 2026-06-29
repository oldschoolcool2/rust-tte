# oracle/30_edge_cases.R
source("oracle/00_setup.R")

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

  # E05 — never initiates: eligible every period, control-only max fan-out.
  # Expect a trial seeded at EVERY period, all assigned=0.
  E05_never_treats = mk(
    id=1, period=0:4, eligible=1, treatment=0, x1=0, x2=0, outcome=0
  ),

  # E07 — eligibility ONLY in the final observed period => single-row trial at the edge.
  E07_last_period_only = mk(
    id=1, period=0:3,
    eligible  =c(0,0,0,1),
    treatment =c(1,1,1,0),  # was treated, then a (contrived) naive final period
    x1=0, x2=0, outcome=0
  )

  # ------------------------------------------------------------------
  # TODO (Mike sign-off + literature alignment): specify these precisely.
  # Each is a known place sequential expansion / time-zero alignment fails.
  #
  # E04_reentry        : eligible -> ineligible -> eligible again.
  #                      Question: does a re-entered eligibility correctly seed a
  #                      NEW trial_period, with assigned_treatment from the re-entry
  #                      period? (immortal-time + re-entry interaction)
  #                      Anchor: Fu/Hernan BMJ 2026 e084909 (time-zero alignment).
  #
  # E06_switch_then_back (PP): A=1 then 0 then 1. Where exactly does PP artificial
  #                      censoring fire, and does it fire on the FIRST deviation only?
  #                      Anchor: Danaei 2013; package expand_until_switch semantics.
  #
  # E08_ties           : simultaneous event/censor on the same visit. Tie-break order?
  #
  # E09_max_fanout     : 1 patient eligible & untreated for many periods (e.g. 0..30)
  #                      => stress row-count invariant + memory in miniature.
  # ------------------------------------------------------------------
)
