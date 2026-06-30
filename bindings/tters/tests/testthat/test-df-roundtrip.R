# Phase-8 in-memory (frame-in / frame-out) round-trip contract test for `tters`.
#
# Reproduces the immutable Oracle fixture battery THROUGH THE IN-MEMORY R BINDING:
# a fixture is read into an R `data.frame` with arrow::read_parquet(), run through
# tters::expand_trial_df() / expand_trial_weighted_df() / fit_trial_weights_df() /
# expand_trial_weighted_fitted_df() (no intermediate Parquet), and the returned
# `data.frame` is compared to the committed `fixtures/expected_*`. The six
# structural columns (id, trial_period, followup_time, assigned_treatment,
# treatment, outcome) must match EXACTLY; `weight` matches within the SAME staged
# tolerances the parquet-path tests use — the *application* path at relative
# 1e-12 (mirror of tests/weights.rs::WEIGHT_REL_TOL) and the *fitted* path at
# 1e-6 (mirror of tests/weights_fit.rs::FITTED_WEIGHT_REL_TOL). Tolerances are
# mirrored from the Rust harness, never invented.
#
# This is the in-memory converse of test-roundtrip.R (parquet apply) and
# test-fit-roundtrip.R (parquet fit). Fixture access mirrors those: the immutable
# battery lives at repo-root `fixtures/`, resolved via $TTERS_FIXTURE_DIR else by
# walking up from the working dir; if neither resolves the tests skip.

WEIGHT_REL_TOL <- 1e-12 # mirror of tests/weights.rs::WEIGHT_REL_TOL (apply path)
FITTED_WEIGHT_REL_TOL <- 1e-6 # mirror of tests/weights_fit.rs::FITTED_WEIGHT_REL_TOL

fixture_dir <- function() {
  env <- Sys.getenv("TTERS_FIXTURE_DIR", "")
  if (nzchar(env) && dir.exists(env)) {
    return(normalizePath(env))
  }
  roots <- c(
    file.path(getwd(), "..", "..", "..", "..", "fixtures"), # tests/testthat -> repo/fixtures
    file.path(getwd(), "fixtures"),
    system.file("extdata", package = "tters")
  )
  for (p in roots) {
    if (nzchar(p) && dir.exists(p) &&
      length(list.files(p, recursive = TRUE, pattern = "\\.parquet$"))) {
      return(normalizePath(p))
    }
  }
  NULL
}

fx <- fixture_dir()

# Read a fixture Parquet into an in-memory data.frame (the in-memory entry points'
# input). int32 -> R integer, double -> R double; the *_df path marshals those
# dtype-exactly back to Polars Int32 / Float64, exactly as scan_parquet would.
rd <- function(rel) as.data.frame(arrow::read_parquet(file.path(fx, rel)))

# Compare an in-memory result `data.frame` against an expected Parquet: column
# names/order + row count + the six structural columns EXACTLY (numeric-equal,
# dtype-agnostic on int vs double storage). Returns the pair for weight checks.
compare_structural_df <- function(actual, expected_path) {
  e <- as.data.frame(arrow::read_parquet(expected_path))
  expect_identical(names(actual), names(e))
  expect_identical(nrow(actual), nrow(e))
  for (col in c(
    "id", "trial_period", "followup_time",
    "assigned_treatment", "treatment", "outcome"
  )) {
    if (col %in% names(e)) {
      expect_identical(
        as.numeric(actual[[col]]), as.numeric(e[[col]]),
        info = sprintf("structural column '%s'", col)
      )
    }
  }
  invisible(list(a = actual, e = e))
}

edge <- c(
  "E01_single", "E02_id4_canonical", "E03_event_at_baseline", "E04_reentry",
  "E05_never_treats", "E06_switch_then_back", "E07_last_period_only",
  "E08_ties", "E09_max_fanout"
)
scen <- c(
  "common", "rare_event", "ultra_rare_event", "rare_initiation",
  "high_switching", "heavy_censoring", "short_followup", "strong_confounding"
)

test_that("fixture battery is reachable", {
  skip_if(is.null(fx), "fixture battery not found (set TTERS_FIXTURE_DIR)")
  expect_true(dir.exists(fx))
})

# ---- Marshalling proves dtype-exact: integer ids stay integer, the E02 double-id
#      cohort stays double, weight is double (the structural dtype contract). ----
test_that("returned data.frame is dtype-exact (int passthrough vs double id)", {
  skip_if(is.null(fx), "no fixtures")
  skip_if_not(file.exists(file.path(fx, "scenarios", "input_common.parquet")), "missing")
  a <- tters::expand_trial_df(rd("scenarios/input_common.parquet"), estimand = "ITT")
  expect_type(a$id, "integer")
  expect_type(a$trial_period, "integer")
  expect_type(a$followup_time, "integer")
  expect_type(a$outcome, "double")

  skip_if_not(file.exists(file.path(fx, "edge", "input_E02_id4_canonical.parquet")), "missing")
  b <- tters::expand_trial_df(rd("edge/input_E02_id4_canonical.parquet"), estimand = "ITT")
  expect_type(b$id, "double") # E02's id is double in the fixture -> passthrough
})

# ---- ITT + PP structural battery (edge E01-E09 + 8 scenarios), in memory. ----
for (nm in c(edge, scen)) {
  local({
    name <- nm
    subdir <- if (name %in% edge) "edge" else "scenarios"
    for (est in c("ITT", "PP")) {
      local({
        estimand <- est
        suffix <- tolower(estimand)
        test_that(sprintf("in-memory %s round-trip: %s/%s", estimand, subdir, name), {
          skip_if(is.null(fx), "no fixtures")
          input <- file.path(fx, subdir, sprintf("input_%s.parquet", name))
          expected <- file.path(fx, subdir, sprintf("expected_%s_%s.parquet", name, suffix))
          skip_if_not(file.exists(input) && file.exists(expected), "fixtures missing")
          a <- tters::expand_trial_df(rd(file.path(subdir, sprintf("input_%s.parquet", name))),
            estimand = estimand
          )
          compare_structural_df(a, expected)
        })
      })
    }
  })
}

# ---- Weighted APPLY battery (cohort frame + factor frame, in memory): structural
#      EXACT + weight within WEIGHT_REL_TOL. ----
weighted <- list(
  list(name = "high_switching", in_sub = "scenarios", est = "PP", wsuf = "pp"),
  list(name = "moderate_switching", in_sub = "scenarios", est = "PP", wsuf = "pp"),
  list(name = "frequent_switching", in_sub = "scenarios", est = "PP", wsuf = "pp"),
  list(name = "data_censored", in_sub = "weights", est = "PP", wsuf = "pp"),
  list(name = "data_censored", in_sub = "weights", est = "ITT", wsuf = "itt")
)
for (w in weighted) {
  local({
    cfg <- w
    test_that(sprintf("in-memory weighted-apply: %s (%s)", cfg$name, cfg$est), {
      skip_if(is.null(fx), "no fixtures")
      input <- file.path(fx, cfg$in_sub, sprintf("input_%s.parquet", cfg$name))
      factors <- file.path(
        fx, "weights",
        sprintf("input_%s_%s_weights.parquet", cfg$name, cfg$wsuf)
      )
      expected <- file.path(
        fx, "weights",
        sprintf("expected_%s_%s_weighted.parquet", cfg$name, cfg$wsuf)
      )
      skip_if_not(
        file.exists(input) && file.exists(factors) && file.exists(expected),
        "weight fixtures missing"
      )
      a <- tters::expand_trial_weighted_df(
        rd(file.path(cfg$in_sub, sprintf("input_%s.parquet", cfg$name))),
        rd(file.path("weights", sprintf("input_%s_%s_weights.parquet", cfg$name, cfg$wsuf))),
        estimand = cfg$est
      )
      res <- compare_structural_df(a, expected)
      rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
      expect_lte(max(rel), WEIGHT_REL_TOL)
    })
  })
}

# ---- Weighted FITTED battery (raw cohort frame -> weighted frame, in memory):
#      structural EXACT + weight within FITTED_WEIGHT_REL_TOL. Specs mirror the
#      canonical crates/tte-expand/tests/weights_fit.rs mapping. ----
fitted <- list(
  list(
    name = "high_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"), cc = NULL, cn = NULL, cd = NULL, pool = "none"
  ),
  list(
    name = "moderate_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"), cc = NULL, cn = NULL, cd = NULL, pool = "none"
  ),
  list(
    name = "frequent_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"), cc = NULL, cn = NULL, cd = NULL, pool = "none"
  ),
  list(
    name = "data_censored", in_sub = "weights", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"),
    cc = "censored", cn = "x2", cd = c("x2", "x1"), pool = "none"
  ),
  list(
    name = "data_censored", in_sub = "weights", est = "ITT", wsuf = "itt",
    sn = NULL, sd = NULL,
    cc = "censored", cn = "x2", cd = "x2", pool = "numerator"
  )
)
for (cfg in fitted) {
  local({
    w <- cfg
    test_that(sprintf("in-memory weighted-fitted: %s (%s)", w$name, w$est), {
      skip_if(is.null(fx), "no fixtures")
      input <- file.path(fx, w$in_sub, sprintf("input_%s.parquet", w$name))
      expected <- file.path(
        fx, "weights",
        sprintf("expected_%s_%s_weighted.parquet", w$name, w$wsuf)
      )
      skip_if_not(file.exists(input) && file.exists(expected), "fit fixtures missing")
      a <- tters::expand_trial_weighted_fitted_df(
        rd(file.path(w$in_sub, sprintf("input_%s.parquet", w$name))),
        estimand = w$est,
        switch_numerator = w$sn, switch_denominator = w$sd,
        censor_col = w$cc, censor_numerator = w$cn, censor_denominator = w$cd,
        pool_censor = w$pool
      )
      res <- compare_structural_df(a, expected)
      rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
      expect_lte(max(rel), FITTED_WEIGHT_REL_TOL)
    })
  })
}

# ---- fit_trial_weights_df returns a factor table that drives the in-memory apply
#      path to the same expected output (a fully in-memory fit -> apply chain). ----
test_that("fit_trial_weights_df table drives expand_trial_weighted_df (high_switching PP)", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "scenarios", "input_high_switching.parquet")
  expected <- file.path(fx, "weights", "expected_high_switching_pp_weighted.parquet")
  skip_if_not(file.exists(input) && file.exists(expected), "fixtures missing")

  cohort <- rd("scenarios/input_high_switching.parquet")
  ft <- tters::fit_trial_weights_df(
    cohort,
    estimand = "PP",
    switch_numerator = "x2", switch_denominator = c("x2", "x1")
  )
  expect_identical(names(ft), c("id", "period", "weight_factor"))
  expect_type(ft$id, "integer")
  expect_type(ft$period, "integer")
  expect_type(ft$weight_factor, "double")
  expect_gt(nrow(ft), 0L)

  a <- tters::expand_trial_weighted_df(cohort, ft, estimand = "PP")
  res <- compare_structural_df(a, expected)
  rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
  expect_lte(max(rel), FITTED_WEIGHT_REL_TOL)
})

# ---- Error mapping: bad inputs surface clean R errors on the in-memory path. ----
test_that("unknown estimand surfaces a clear R error (in-memory)", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "edge", "input_E01_single.parquet")
  skip_if_not(file.exists(input), "E01 input missing")
  expect_error(
    tters::expand_trial_df(rd("edge/input_E01_single.parquet"), estimand = "BOGUS"),
    "unknown estimand"
  )
})

test_that("unknown pool_censor surfaces a clear R error (in-memory)", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "weights", "input_data_censored.parquet")
  skip_if_not(file.exists(input), "data_censored input missing")
  expect_error(
    tters::expand_trial_weighted_fitted_df(
      rd("weights/input_data_censored.parquet"),
      estimand = "ITT",
      censor_col = "censored", censor_numerator = "x2", censor_denominator = "x2",
      pool_censor = "bogus"
    ),
    "unknown pool_censor"
  )
})
