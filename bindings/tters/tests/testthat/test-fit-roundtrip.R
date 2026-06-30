# Phase-7 fit-round-trip contract test for the `tters` extendr binding.
#
# Reproduces the 5 immutable weight fixtures THROUGH THE R BINDING BY *FITTING*
# (not by applying a pre-computed factor table): tters::expand_trial_weighted_fitted()
# fits the IPW switching / IPCW-censoring models in Rust, expands, applies, and
# writes Parquet, which is read back and compared to the committed
# `fixtures/weights/expected_*_weighted.parquet`. The six structural columns
# (id, trial_period, followup_time, assigned_treatment, treatment, outcome) must
# match EXACTLY; the cumulative `weight` matches within the staged FITTED tolerance
# mirrored from the Rust harness
# (crates/tte-expand/tests/weights_fit.rs::FITTED_WEIGHT_REL_TOL = 1e-6, NOT the
# 1e-12 *application* tolerance used by test-roundtrip.R).
#
# Fixture access mirrors test-roundtrip.R: the immutable battery lives at repo-root
# `fixtures/`, resolved via $TTERS_FIXTURE_DIR else by walking up from the working
# dir; if neither resolves the tests skip.

# Mirror of tests/weights_fit.rs::FITTED_WEIGHT_REL_TOL (do not invent). Fitting is
# L-BFGS-to-MLE, not bit-for-bit IRLS, so `weight` is staged at ~1e-6 (ADR-2);
# structural columns stay exact.
FITTED_WEIGHT_REL_TOL <- 1e-6

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

# Compare two Parquet frames: column names/order + row count + the six structural
# columns EXACTLY (numeric-equal, dtype-agnostic on int vs double storage).
compare_structural <- function(actual_path, expected_path) {
  a <- arrow::read_parquet(actual_path)
  e <- arrow::read_parquet(expected_path)
  expect_identical(names(a), names(e))
  expect_identical(nrow(a), nrow(e))
  for (col in c(
    "id", "trial_period", "followup_time",
    "assigned_treatment", "treatment", "outcome"
  )) {
    if (col %in% names(e)) {
      expect_identical(
        as.numeric(a[[col]]), as.numeric(e[[col]]),
        info = sprintf("structural column '%s'", col)
      )
    }
  }
  invisible(list(a = a, e = e))
}

# The 5 fixtures, fitted from their raw cohorts. Switching scenarios read from
# `scenarios/`; data_censored reads from `weights/`. Specs mirror the canonical
# crates/tte-expand/tests/weights_fit.rs mapping (n ~ x2, d ~ x2 + x1 for switching;
# IPCW on `censored`; ITT pools the numerator across am_1 strata).
fitted <- list(
  list(
    name = "high_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"),
    cc = NULL, cn = NULL, cd = NULL, pool = "none"
  ),
  list(
    name = "moderate_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"),
    cc = NULL, cn = NULL, cd = NULL, pool = "none"
  ),
  list(
    name = "frequent_switching", in_sub = "scenarios", est = "PP", wsuf = "pp",
    sn = "x2", sd = c("x2", "x1"),
    cc = NULL, cn = NULL, cd = NULL, pool = "none"
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

test_that("fixture battery is reachable", {
  skip_if(is.null(fx), "fixture battery not found (set TTERS_FIXTURE_DIR)")
  expect_true(dir.exists(fx))
})

# ---- Fitted weighted battery: structural EXACT + weight within FITTED tol. ----
for (cfg in fitted) {
  local({
    w <- cfg
    test_that(sprintf("fitted round-trip: %s (%s)", w$name, w$est), {
      skip_if(is.null(fx), "no fixtures")
      input <- file.path(fx, w$in_sub, sprintf("input_%s.parquet", w$name))
      expected <- file.path(
        fx, "weights",
        sprintf("expected_%s_%s_weighted.parquet", w$name, w$wsuf)
      )
      skip_if_not(
        file.exists(input) && file.exists(expected),
        "fit fixtures missing"
      )
      out <- tempfile(fileext = ".parquet")
      on.exit(unlink(out), add = TRUE)
      tters::expand_trial_weighted_fitted(
        input, out,
        estimand = w$est,
        switch_numerator = w$sn, switch_denominator = w$sd,
        censor_col = w$cc, censor_numerator = w$cn, censor_denominator = w$cd,
        pool_censor = w$pool
      )
      res <- compare_structural(out, expected)
      rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
      expect_lte(max(rel), FITTED_WEIGHT_REL_TOL)
    })
  })
}

# ---- fit_trial_weights writes a factor table the apply path can consume. ----
test_that("fit_trial_weights factor table drives the apply path (high_switching PP)", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "scenarios", "input_high_switching.parquet")
  expected <- file.path(fx, "weights", "expected_high_switching_pp_weighted.parquet")
  skip_if_not(file.exists(input) && file.exists(expected), "fixtures missing")
  factors <- tempfile(fileext = ".parquet")
  weighted <- tempfile(fileext = ".parquet")
  on.exit(unlink(c(factors, weighted)), add = TRUE)

  tters::fit_trial_weights(
    input, factors,
    estimand = "PP",
    switch_numerator = "x2", switch_denominator = c("x2", "x1")
  )
  ft <- arrow::read_parquet(factors)
  expect_identical(names(ft), c("id", "period", "weight_factor"))
  expect_gt(nrow(ft), 0L)

  # The fitted factor table must drive the Phase-3 application path to the same
  # expected output, within the fitted tolerance.
  tters::expand_trial_weighted(input, factors, weighted, estimand = "PP")
  res <- compare_structural(weighted, expected)
  rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
  expect_lte(max(rel), FITTED_WEIGHT_REL_TOL)
})

# ---- Error mapping: a bad spec surfaces a clean R error (not a crash). ----
test_that("unknown pool_censor surfaces a clear R error", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "weights", "input_data_censored.parquet")
  skip_if_not(file.exists(input), "data_censored input missing")
  out <- tempfile(fileext = ".parquet")
  on.exit(unlink(out), add = TRUE)
  expect_error(
    tters::expand_trial_weighted_fitted(
      input, out,
      estimand = "ITT",
      censor_col = "censored", censor_numerator = "x2", censor_denominator = "x2",
      pool_censor = "bogus"
    ),
    "unknown pool_censor"
  )
})
