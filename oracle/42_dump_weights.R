# oracle/42_dump_weights.R  (Phase 3 — weight fixtures)
#
# Emits, for each (cohort, estimand) weight case, TWO fixtures under
# fixtures/weights/:
#   * expected_<name>_<estimand>_weighted.parquet  — the R-Oracle weighted
#     expanded frame: the six STRUCTURAL_COLS + a `weight` (Float64) column,
#     re-ordered to (id, trial_period, followup_time, assigned_treatment,
#     treatment, outcome, weight) and sorted by (id, trial_period,
#     followup_time). The structural columns are matched EXACTLY; `weight` is
#     matched within the harness tolerance (~1e-12 relative, ADR-2).
#   * input_<name>_<estimand>_weights.parquet — the per-(id,period) IPW *factor*
#     the engine joins: columns (id, period [Int32], weight_factor [Float64]).
#     This is R's per-period stabilised weight contribution, recovered as the
#     trial-invariant ratio weight[t]/weight[t-1] (proven invariant across the
#     overlapping trials that share an (id, period)). The engine reproduces
#     `weight` by joining this factor on (id, period := trial_period +
#     followup_time) and taking the cumulative product within (id, trial_period)
#     ordered by followup_time, with the baseline (followup_time==0) multiplier
#     forced to 1.0.
#
# Weights come from the LEGACY data_preparation(use_censor_weights=...) path —
# the only path that emits real (non-placeholder) weights on TrialEmulation
# 0.0.4.11 (the S4 calculate_weights() path returned weight==1.0). The legacy PP
# row-set equals the S4 PP row-set (verified setequal on data_censored), which
# equals the engine's PP expansion, so the weighted frame aligns row-for-row with
# the engine's structural output.
#
# Only cohorts with ACTUAL treatment switching (high_switching, the new
# moderate_/frequent_switching scenarios, and data_censored) can carry switch
# weights; data_censored is the only cohort with an explicit `censored` column,
# hence the only source of IPCW. All five configs fit cleanly (glm warn = 0).
#
# Run from repo root:  Rscript oracle/42_dump_weights.R
#   (pre-flight dry-run: TTE_FIXTURES_OUT=<scratch> [TTE_SCENARIOS_SRC=<file>] Rscript ...)
source(Sys.getenv("TTE_SCENARIOS_SRC", "oracle/20_scenarios.R"))  # SCENARIOS, simulate_cohort, consts

# Committed inputs are READ from IN_ROOT; all fixtures are WRITTEN to OUT_ROOT.
# Both default to "fixtures"; TTE_FIXTURES_OUT redirects writes for a no-side-
# effect dry-run.
IN_ROOT  <- OUT_ROOT
OUT_ROOT <- Sys.getenv("TTE_FIXTURES_OUT", OUT_ROOT)
# Defined in 00_setup.R; defensive fallback keeps this script runnable standalone.
if (!exists("STRUCTURAL_COLS_WEIGHTED"))
  STRUCTURAL_COLS_WEIGHTED <- c(STRUCTURAL_COLS, "weight")

# Run the legacy weight path; return the weighted frame in the canonical
# STRUCTURAL_COLS_WEIGHTED order, sorted by (id, trial_period, followup_time).
weight_expand <- function(cohort, estimand, args = list()) {
  td <- tempfile("tew_"); dir.create(td); on.exit(unlink(td, recursive = TRUE), add = TRUE)
  call_args <- c(list(
    data = cohort, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible", estimand_type = estimand,
    outcome_cov = ~1, model_var = "assigned_treatment",
    data_dir = td, separate_files = FALSE, quiet = TRUE
  ), args)
  prep <- do.call(data_preparation, call_args)
  d <- as.data.frame(prep$data)
  miss <- setdiff(STRUCTURAL_COLS_WEIGHTED, names(d))
  if (length(miss)) stop("weighted frame missing columns: ", paste(miss, collapse = ", "))
  d <- d[, STRUCTURAL_COLS_WEIGHTED, drop = FALSE]
  d[do.call(order, d[c("id", "trial_period", "followup_time")]), , drop = FALSE]
}

# Recover the per-(id,period) IPW factor from the weighted frame: the
# trial-invariant ratio weight[t]/weight[t-1]; baseline rows define no factor
# (their multiplier is 1.0). Returns (id, period [int], weight_factor [double]).
derive_factors <- function(weighted) {
  dt <- as.data.table(weighted)[order(id, trial_period, followup_time)]
  dt[, period := as.integer(trial_period + followup_time)]
  dt[, prevw := shift(weight, 1, type = "lag"), by = .(id, trial_period)]
  dt[, fac := weight / prevw]
  f <- dt[followup_time > 0, .(weight_factor = fac[1L], spread = max(fac) - min(fac)),
          by = .(id, period)]
  if (nrow(f) && max(f$spread) > 1e-10)
    stop("per-(id,period) factor NOT trial-invariant; max spread = ", max(f$spread))
  as.data.frame(f[order(id, period), .(id, period, weight_factor)])
}

# (name, estimand, source, weight args). source: "committed" (read
# fixtures/scenarios/input_<name>.parquet), "simulate" (regenerate from the
# SCENARIOS registry and commit the input), or "data_censored" (bundled dataset).
CASES <- list(
  list(name = "high_switching",     est = "PP",  src = "committed",
       args = list(switch_n_cov = ~x2, switch_d_cov = ~x2 + x1, use_censor_weights = FALSE)),
  list(name = "moderate_switching", est = "PP",  src = "simulate",
       args = list(switch_n_cov = ~x2, switch_d_cov = ~x2 + x1, use_censor_weights = FALSE)),
  list(name = "frequent_switching", est = "PP",  src = "simulate",
       args = list(switch_n_cov = ~x2, switch_d_cov = ~x2 + x1, use_censor_weights = FALSE)),
  list(name = "data_censored",      est = "PP",  src = "data_censored",
       args = list(switch_n_cov = ~x2, switch_d_cov = ~x2 + x1, use_censor_weights = TRUE,
                   cense = "censored", cense_n_cov = ~x2, cense_d_cov = ~x2 + x1, pool_cense = "none")),
  list(name = "data_censored",      est = "ITT", src = "data_censored",
       args = list(use_censor_weights = TRUE, cense = "censored",
                   cense_n_cov = ~x2, cense_d_cov = ~x2, pool_cense = "numerator"))
)

scen_dir <- file.path(OUT_ROOT, "scenarios"); dir.create(scen_dir, recursive = TRUE, showWarnings = FALSE)
out_dir  <- file.path(OUT_ROOT, "weights");   dir.create(out_dir,  recursive = TRUE, showWarnings = FALSE)
data(data_censored)
dc_in <- file.path(out_dir, "input_data_censored.parquet")
arrow::write_parquet(data_censored, dc_in)
entries <- list(list(role = "input", path = dc_in,
                     sha256 = digest(file = dc_in, algo = "sha256"), n_rows = nrow(data_censored)))
summ <- data.frame()

for (cs in CASES) {
  cohort <- switch(cs$src,
    committed     = as.data.frame(arrow::read_parquet(
                      file.path(IN_ROOT, "scenarios", sprintf("input_%s.parquet", cs$name)))),
    data_censored = data_censored,
    simulate      = {
      s <- SCENARIOS[[cs$name]]; stopifnot(!is.null(s))
      # simulate_cohort directly (skip validate_input — the data.table by= bug);
      # deterministic given the fixed seed. Commit the structural input.
      co <- simulate_cohort(s$n, s$max_period, s$params, s$seed)
      ip <- file.path(scen_dir, sprintf("input_%s.parquet", cs$name))
      arrow::write_parquet(co, ip)
      entries[[length(entries) + 1]] <- list(role = "input", path = ip,
        sha256 = digest(file = ip, algo = "sha256"), n_rows = nrow(co))
      co
    })
  w   <- weight_expand(cohort, cs$est, cs$args)
  f   <- derive_factors(w)
  est <- tolower(cs$est)
  wp  <- file.path(out_dir, sprintf("expected_%s_%s_weighted.parquet", cs$name, est))
  fp  <- file.path(out_dir, sprintf("input_%s_%s_weights.parquet", cs$name, est))
  arrow::write_parquet(w, wp)
  arrow::write_parquet(f, fp)
  entries[[length(entries) + 1]] <- list(role = paste0("expected_", est, "_weighted"), path = wp,
                                          sha256 = digest(file = wp, algo = "sha256"), n_rows = nrow(w))
  entries[[length(entries) + 1]] <- list(role = paste0("input_", est, "_weights"), path = fp,
                                          sha256 = digest(file = fp, algo = "sha256"), n_rows = nrow(f))
  summ <- rbind(summ, data.frame(case = cs$name, estimand = cs$est, rows = nrow(w), factors = nrow(f),
                                 w_min = round(min(w$weight), 5), w_max = round(max(w$weight), 5)))
}

jsonlite::write_json(list(provenance = ORACLE_PROVENANCE, fixtures = entries),
                     file.path(out_dir, "MANIFEST_weights.json"), pretty = TRUE, auto_unbox = TRUE)
cat("\n==== Phase-3 weight fixtures written under '", out_dir, "' ====\n", sep = "")
print(summ, row.names = FALSE)
