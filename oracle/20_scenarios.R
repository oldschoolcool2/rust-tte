# oracle/20_scenarios.R
source("oracle/10_simulate.R")

SCENARIOS <- list(
  common = list(
    desc = "Workhorse: moderate initiation, common events, light censoring.",
    n = 300, max_period = 12, seed = 101,
    params = list(init_int = -1.5, out_int = -2.5, cens_prob = 0.03)
  ),
  rare_event = list(
    desc = "Common exposure, rare outcome (typical pharmacoepi safety study).",
    n = 800, max_period = 18, seed = 102,
    params = list(init_int = -1.5, out_int = -4.5, cens_prob = 0.03)
  ),
  ultra_rare_event = list(
    desc = "Ultra-rare outcome; many trials, almost no events (numerics stress).",
    n = 2000, max_period = 18, seed = 103,
    params = list(init_int = -1.5, out_int = -6.5, cens_prob = 0.03)
  ),
  rare_initiation = list(
    desc = "Treatment seldom started => long recurrent-eligibility runs per id.",
    n = 600, max_period = 18, seed = 104,
    params = list(init_int = -3.5, out_int = -3.0, cens_prob = 0.03)
  ),
  high_switching = list(
    desc = "Frequent deviation => maximal ITT-vs-PP divergence.",
    n = 400, max_period = 15, seed = 105,
    params = list(init_int = -1.0, switch_prob = 0.25, out_int = -3.0)
  ),
  heavy_censoring = list(
    desc = "High dropout => short, ragged follow-up; many truncated trials.",
    n = 500, max_period = 15, seed = 106,
    params = list(cens_prob = 0.20, out_int = -3.0)
  ),
  short_followup = list(
    desc = "Few periods => many single-/few-row trials.",
    n = 400, max_period = 3, seed = 107,
    params = list(init_int = -1.0, out_int = -2.5)
  ),
  strong_confounding = list(
    desc = "Strong L->A and L->Y => stresses downstream weighting (Tier 2/3).",
    n = 600, max_period = 15, seed = 108,
    params = list(conf_AL = 1.6, conf_YL = 1.6, out_int = -3.0)
  ),
  large_scale = list(
    desc = "Volume/memory shakeout (use for benchmarks, not unit fixtures).",
    n = 20000, max_period = 24, seed = 109,
    params = list(init_int = -1.5, out_int = -3.5)
  )
)

build_scenario <- function(name) {
  s <- SCENARIOS[[name]]
  stopifnot(!is.null(s))
  validate_input(simulate_cohort(s$n, s$max_period, s$params, s$seed))
}
