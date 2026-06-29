# R round-trip contract test for the `tters` extendr binding (Phase 4).
#
# Reproduces the immutable Oracle fixture battery THROUGH THE R BINDING:
# tters::expand_trial() / expand_trial_weighted() write Parquet, which is read
# back and compared to the committed `fixtures/expected_*`. The six structural
# columns (id, trial_period, followup_time, assigned_treatment, treatment,
# outcome) must match EXACTLY; the `weight` column matches within the SAME
# relative tolerance the Rust harness uses
# (crates/tte-expand/tests/weights.rs::WEIGHT_REL_TOL = 1e-12) — mirrored here,
# not invented.
#
# Fixture access: the immutable battery lives at repo-root `fixtures/`. Resolved
# via $TTERS_FIXTURE_DIR, else by walking up from the working dir; if neither
# resolves (e.g. an installed package with no repo alongside) the tests skip.
# Vendoring a subset into inst/extdata for a fully self-contained installed
# self-test is deferred — see docs/006-phase-4-extendr-binding.

WEIGHT_REL_TOL <- 1e-12 # mirror of tests/weights.rs::WEIGHT_REL_TOL (do not invent)

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

expand_one <- function(name, subdir, estimand, suffix) {
  input <- file.path(fx, subdir, sprintf("input_%s.parquet", name))
  expected <- file.path(fx, subdir, sprintf("expected_%s_%s.parquet", name, suffix))
  skip_if_not(
    file.exists(input) && file.exists(expected),
    sprintf("fixtures for %s/%s missing", subdir, name)
  )
  out <- tempfile(fileext = ".parquet")
  on.exit(unlink(out), add = TRUE)
  tters::expand_trial(input, out, estimand = estimand)
  compare_structural(out, expected)
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

test_that("unknown estimand surfaces a clear R error", {
  skip_if(is.null(fx), "no fixtures")
  input <- file.path(fx, "edge", "input_E01_single.parquet")
  skip_if_not(file.exists(input), "E01 input missing")
  out <- tempfile(fileext = ".parquet")
  on.exit(unlink(out), add = TRUE)
  expect_error(
    tters::expand_trial(input, out, estimand = "BOGUS"),
    "unknown estimand"
  )
})

# ---- ITT + PP structural battery (edge E01-E09 + 8 scenarios). ----
for (nm in c(edge, scen)) {
  local({
    name <- nm
    subdir <- if (name %in% edge) "edge" else "scenarios"
    test_that(sprintf("ITT round-trip: %s/%s", subdir, name), {
      skip_if(is.null(fx), "no fixtures")
      expand_one(name, subdir, "ITT", "itt")
    })
    test_that(sprintf("PP round-trip: %s/%s", subdir, name), {
      skip_if(is.null(fx), "no fixtures")
      expand_one(name, subdir, "PP", "pp")
    })
  })
}

# ---- Weighted battery: structural EXACT + weight within WEIGHT_REL_TOL. ----
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
    test_that(sprintf("weighted round-trip: %s (%s)", cfg$name, cfg$est), {
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
      out <- tempfile(fileext = ".parquet")
      on.exit(unlink(out), add = TRUE)
      tters::expand_trial_weighted(input, factors, out, estimand = cfg$est)
      res <- compare_structural(out, expected)
      rel <- abs(res$a$weight - res$e$weight) / pmax(abs(res$e$weight), 1)
      expect_lte(max(rel), WEIGHT_REL_TOL)
    })
  })
}
