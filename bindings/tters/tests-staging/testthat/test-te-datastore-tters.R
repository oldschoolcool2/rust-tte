# Phase-11 contract test for the `te_datastore` companion backend.
#
# Proves, on TrialEmulation's bundled `data_censored` cohort, that running the
# expansion in Rust via `expand_trials_tters()` and presenting it through the
# `te_datastore_tters` backend is transparent versus the default
# `te_datastore_datatable` path:
#   - D1: the S4 backend (save_to_tters / save_expanded_data / read_expanded_data
#         / show) conforms to TrialEmulation's te_datastore contract.
#   - D2: the stored expanded frame is byte-equivalent to the default path —
#         structural columns EXACT, `weight` within the apply-path tolerance.
#   - D3: downstream load + seeded sampling + fit_msm coefficients match.
#   - D4: graceful fallback (unsupported AT estimand falls back to R; the Rust
#         path can be forced with fallback = FALSE).
#
# The whole suite SKIPS when TrialEmulation (or data.table) is not installed, so
# plain `tters` is unaffected. Tolerances mirror the existing tters harness:
# `weight` at the apply-path relative 1e-12 (tests/weights.rs::WEIGHT_REL_TOL);
# fit_msm coefficients agree to a slack 1e-6 (glm float noise from a ~1e-16
# weight perturbation).

WEIGHT_REL_TOL <- 1e-12 # mirror of tests/weights.rs::WEIGHT_REL_TOL (apply path)
COEF_TOL <- 1e-6 # glm coefficient slack (downstream estimation in R)

te_available <- function() {
  requireNamespace("TrialEmulation", quietly = TRUE) &&
    requireNamespace("data.table", quietly = TRUE)
}

# Build a configured (un-expanded) trial_sequence on data_censored.
build_seq <- function(estimand = c("ITT", "PP"), weighted = TRUE) {
  estimand <- match.arg(estimand)
  data_censored <- TrialEmulation::data_censored
  ts <- TrialEmulation::trial_sequence(estimand) |>
    TrialEmulation::set_data(
      data = data_censored, id = "id", period = "period",
      treatment = "treatment", outcome = "outcome", eligible = "eligible"
    )
  if (estimand == "PP") {
    ts <- ts |>
      TrialEmulation::set_switch_weight_model(
        numerator = ~age, denominator = ~ age + x1 + x3,
        model_fitter = TrialEmulation::stats_glm_logit(save_path = tempfile("sw_"))
      )
  }
  if (weighted) {
    ts <- ts |>
      TrialEmulation::set_censor_weight_model(
        censor_event = "censored", numerator = ~x2, denominator = ~ x2 + x1,
        pool_models = if (estimand == "PP") "none" else "numerator",
        model_fitter = TrialEmulation::stats_glm_logit(save_path = tempfile("cw_"))
      )
    ts <- suppressWarnings(TrialEmulation::calculate_weights(ts))
  }
  if (estimand == "ITT") {
    TrialEmulation::set_outcome_model(ts, adjustment_terms = ~x2)
  } else {
    TrialEmulation::set_outcome_model(ts)
  }
}

# Expand `template` both ways and return the two stored data.tables.
expand_both <- function(template) {
  ref <- TrialEmulation::set_expansion_options(
    template,
    output = TrialEmulation::save_to_datatable(), chunk_size = 500
  )
  ref <- TrialEmulation::expand_trials(ref)
  tt <- TrialEmulation::set_expansion_options(
    template,
    output = save_to_tters(), chunk_size = 0
  )
  tt <- expand_trials_tters(tt, fallback = FALSE)
  list(
    ref_seq = ref, tt_seq = tt,
    ref = ref@expansion@datastore@data,
    got = tt@expansion@datastore@data
  )
}

# Assert two stored frames are equivalent: identical schema/order, structural
# columns exact, `weight` within the apply-path relative tolerance.
expect_frame_equiv <- function(got, ref) {
  expect_identical(colnames(got), colnames(ref))
  expect_identical(vapply(got, class, ""), vapply(ref, class, ""))
  expect_identical(nrow(got), nrow(ref))
  for (col in setdiff(colnames(ref), "weight")) {
    expect_identical(
      got[[col]], ref[[col]],
      info = sprintf("structural column '%s'", col)
    )
  }
  rel <- abs(got[["weight"]] - ref[["weight"]]) / pmax(abs(ref[["weight"]]), 1)
  expect_lte(max(rel), WEIGHT_REL_TOL)
}

test_that("save_to_tters() builds a conformant te_datastore (D1)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  store <- save_to_tters()
  expect_s4_class(store, "te_datastore_tters")
  expect_true(methods::is(store, "te_datastore"))
  expect_identical(store@N, 0L)
  expect_type(store@N, "integer")
  expect_output(show(store), "tters")
})

test_that("ITT unweighted: stored frame is byte-equivalent to datatable (D2)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  b <- expand_both(build_seq("ITT", weighted = FALSE))
  expect_frame_equiv(b$got, b$ref)
  # unweighted => weight is exactly 1
  expect_true(all(b$got[["weight"]] == 1))
})

test_that("ITT weighted (IPCW): stored frame is byte-equivalent (D2)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  b <- expand_both(build_seq("ITT", weighted = TRUE))
  expect_frame_equiv(b$got, b$ref)
  expect_true("assigned_treatment" %in% colnames(b$got))
  expect_true("x2" %in% colnames(b$got))
})

test_that("PP weighted (switch + censor): stored frame is byte-equivalent (D2)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  b <- expand_both(build_seq("PP", weighted = TRUE))
  expect_frame_equiv(b$got, b$ref)
})

test_that("read_expanded_data honours period + subset_condition (D1)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  b <- expand_both(build_seq("ITT", weighted = TRUE))
  store <- b$tt_seq@expansion@datastore

  all_rows <- read_expanded_data(store, period = NULL, subset_condition = NULL)
  expect_identical(nrow(all_rows), b$tt_seq@expansion@datastore@N)

  p1 <- read_expanded_data(store, period = 1L, subset_condition = NULL)
  expect_true(all(p1$trial_period == 1L))
  expect_identical(nrow(p1), sum(b$got$trial_period == 1L))

  sub <- read_expanded_data(store, period = NULL, subset_condition = "x2 > 0")
  expect_true(all(sub$x2 > 0))
  expect_identical(nrow(sub), sum(b$got$x2 > 0))

  expect_error(
    read_expanded_data(store, period = -1L, subset_condition = NULL),
    "non-negative"
  )
})

test_that("downstream load + seeded sampling + fit_msm match the default path (D3)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  b <- expand_both(build_seq("ITT", weighted = TRUE))

  # load without sampling: identical outcome data (values)
  od_ref <- TrialEmulation::outcome_data(TrialEmulation::load_expanded_data(b$ref_seq))
  od_tt <- TrialEmulation::outcome_data(TrialEmulation::load_expanded_data(b$tt_seq))
  expect_equal(as.data.frame(od_tt), as.data.frame(od_ref), tolerance = WEIGHT_REL_TOL)

  # seeded case-control sampling: identical sampled rows
  sd_ref <- TrialEmulation::load_expanded_data(b$ref_seq, seed = 1234L, p_control = 0.5)
  sd_tt <- TrialEmulation::load_expanded_data(b$tt_seq, seed = 1234L, p_control = 0.5)
  s_ref <- TrialEmulation::outcome_data(sd_ref)
  s_tt <- TrialEmulation::outcome_data(sd_tt)
  expect_identical(nrow(s_tt), nrow(s_ref))
  expect_equal(as.data.frame(s_tt), as.data.frame(s_ref), tolerance = WEIGHT_REL_TOL)

  # fit_msm coefficients agree
  fm_ref <- suppressWarnings(TrialEmulation::fit_msm(sd_ref, weight_cols = c("weight", "sample_weight")))
  fm_tt <- suppressWarnings(TrialEmulation::fit_msm(sd_tt, weight_cols = c("weight", "sample_weight")))
  co_ref <- coef(fm_ref@outcome_model@fitted@model$model)
  co_tt <- coef(fm_tt@outcome_model@fitted@model$model)
  expect_identical(names(co_tt), names(co_ref))
  expect_lte(max(abs(co_tt - co_ref)), COEF_TOL)
})

test_that("AT estimand gracefully falls back to R (D4)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  data_censored <- TrialEmulation::data_censored
  at <- TrialEmulation::trial_sequence("AT") |>
    TrialEmulation::set_data(
      data = data_censored, id = "id", period = "period",
      treatment = "treatment", outcome = "outcome", eligible = "eligible"
    ) |>
    TrialEmulation::set_outcome_model() |>
    TrialEmulation::set_expansion_options(
      output = save_to_tters(), chunk_size = 0, censor_at_switch = FALSE
    )
  expect_message(
    done <- expand_trials_tters(at, fallback = TRUE),
    "falling back"
  )
  expect_true(methods::is(done, "trial_sequence"))
  expect_gt(done@expansion@datastore@N, 0L)

  # with fallback disabled the unsupported estimand is an error
  expect_error(expand_trials_tters(at, fallback = FALSE), "AT estimand")
})

test_that("expand_trials_tters rejects a non-trial_sequence (fallback = FALSE)", {
  skip_if_not(te_available(), "TrialEmulation/data.table not installed")
  expect_error(expand_trials_tters(data.frame(x = 1), fallback = FALSE), "trial_sequence")
})
