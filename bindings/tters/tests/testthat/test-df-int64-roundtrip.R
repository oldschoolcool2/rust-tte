# Phase-9 exact wide-integer (bit64::integer64) in-memory round-trip for `tters`.
#
# Phase 8 made the *_df entry points go frame-in / frame-out for base-R `integer`
# (Int32) and `double` (Float64). Phase 9 closes the 64-bit gap: an R
# `bit64::integer64` column marshals to a Polars Int64 and back EXACTLY, by a
# pure-safe bit reinterpret (i64 <-> the f64 carrying its bit pattern) — no
# `unsafe`, no Arrow C Data Interface, and crucially NO precision loss above 2^53
# (where a naive numeric double-cast would silently corrupt ids).
#
# Strategy: a "shifted twin". A base integer cohort is run through the *_df path
# both as-is and with its `id` lifted to integer64 and shifted by 2^53 (so every
# id exceeds 2^53 and is no longer representable as a double). The two runs must
# agree on every structural column and on `weight`, the integer64 run must return
# an `integer64`-classed `id`, and that id must equal the plain id + 2^53 EXACTLY
# (bit64). A naive cast would collapse 2^53+1 -> 2^53, breaking that equality and
# (in the collision case) merging two distinct ids — so these assertions are a
# genuine precision contract, not a tautology. The cohorts here are synthesized
# test inputs; no fixture is added.

skip_if_no_bit64 <- function() skip_if_not_installed("bit64")

# 2^53 as an integer64; the shift that pushes ids past double precision. Built
# from a STRING so the literal is not rounded to a double before conversion.
offset64 <- function() bit64::as.integer64("9007199254740992")

# Decimal strings of an (integer-or-integer64) vector, for exact, dispatch-safe
# comparison (avoids relying on attached-vs-loaded bit64 operator masking).
as_dec <- function(x) as.character(bit64::as.integer64(x))

# Fixture battery locator (mirrors test-df-roundtrip.R): repo-root `fixtures/`,
# resolved via $TTERS_FIXTURE_DIR else by walking up from the working dir.
fixture_dir <- function() {
  env <- Sys.getenv("TTERS_FIXTURE_DIR", "")
  if (nzchar(env) && dir.exists(env)) {
    return(normalizePath(env))
  }
  roots <- c(
    file.path(getwd(), "..", "..", "..", "..", "fixtures"),
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
rd <- function(rel) as.data.frame(arrow::read_parquet(file.path(fx, rel)))

# A small multi-trial cohort with a treatment switch (id 1: 1,1,0,1) so the PP
# estimand genuinely diverges from ITT, and a baseline event (id 2). Plain int.
base_cohort <- function() {
  data.frame(
    id        = c(1L, 1L, 1L, 1L, 2L, 2L, 2L),
    period    = c(0L, 1L, 2L, 3L, 0L, 1L, 2L),
    treatment = c(1L, 1L, 0L, 1L, 0L, 0L, 0L),
    eligible  = c(1L, 1L, 1L, 0L, 1L, 1L, 0L),
    outcome   = c(0, 0, 0, 0, 0, 0, 1)
  )
}

# Assert two row-aligned frames agree on the non-id structural columns (numeric
# equality, dtype-agnostic on int vs integer64 vs double storage).
expect_structural_parity <- function(a, b, cols) {
  expect_identical(nrow(a), nrow(b))
  for (col in cols) {
    expect_identical(
      as.numeric(a[[col]]), as.numeric(b[[col]]),
      info = sprintf("structural column '%s'", col)
    )
  }
}

# ---- expand_trial_df: integer64 id round-trips exactly (ITT + PP). ----
for (est in c("ITT", "PP")) {
  local({
    estimand <- est
    test_that(sprintf("expand_trial_df: integer64 id round-trips exactly (%s)", estimand), {
      skip_if_no_bit64()
      base <- base_cohort()
      co64 <- base
      co64$id <- bit64::as.integer64(base$id) + offset64() # 2^53+1, 2^53+2 (> 2^53)

      r_plain <- tters::expand_trial_df(base, estimand = estimand)
      r_64 <- tters::expand_trial_df(co64, estimand = estimand)

      # Storage class: integer64 in -> integer64 out; the plain path is unchanged.
      expect_s3_class(r_64$id, "integer64")
      expect_type(r_plain$id, "integer")
      expect_identical(names(r_64), names(r_plain))

      # Non-id structural columns identical between the twins.
      expect_structural_parity(
        r_64, r_plain,
        c("trial_period", "followup_time", "assigned_treatment", "treatment", "outcome")
      )

      # id round-trips EXACTLY: r_64$id == r_plain$id + 2^53 (bit64, no precision loss).
      expect_identical(as_dec(r_64$id), as_dec(bit64::as.integer64(r_plain$id) + offset64()))

      # The precision-critical value 2^53+1 (not f64-representable) must survive; a
      # naive double-cast marshaller would return 2^53 here and fail the line above.
      expect_true("9007199254740993" %in% as_dec(r_64$id))

      # Distinct input ids stay distinct (no collapse under the cast).
      expect_identical(
        length(unique(as_dec(r_64$id))),
        length(unique(base$id))
      )
    })
  })
}

# ---- Distinctness: two adjacent ids past 2^53 (a naive cast collapses both to
#      2^53) must stay separate. ----
test_that("expand_trial_df: adjacent >2^53 ids stay distinct (no cast collision)", {
  skip_if_no_bit64()
  co <- data.frame(
    period = c(0L, 0L), treatment = c(0L, 0L), eligible = c(1L, 1L), outcome = c(0, 0)
  )
  co$id <- bit64::as.integer64(c("9007199254740992", "9007199254740993")) # 2^53, 2^53+1

  out <- tters::expand_trial_df(co, estimand = "ITT")
  expect_s3_class(out$id, "integer64")
  # One baseline row per id; both ids preserved distinctly (a naive cast -> 1 id).
  expect_identical(
    sort(unique(as_dec(out$id))),
    c("9007199254740992", "9007199254740993")
  )
  expect_identical(nrow(out), 2L)
})

# ---- integer64 id AND period AND treatment in: exercises the input arm for each
#      and the output arm for the columns that inherit those dtypes. trial_period
#      is ALWAYS Int32 (the core casts it), so it returns plain `integer`. ----
test_that("expand_trial_df: integer64 id+period+treatment marshals per the dtype contract", {
  skip_if_no_bit64()
  base <- base_cohort()
  co64 <- base
  co64$id <- bit64::as.integer64(base$id) + offset64()
  co64$period <- bit64::as.integer64(base$period)
  co64$treatment <- bit64::as.integer64(base$treatment)

  out <- tters::expand_trial_df(co64, estimand = "ITT")
  # period -> Int64 makes followup_time Int64; treatment passes through Int64.
  expect_s3_class(out$id, "integer64")
  expect_s3_class(out$followup_time, "integer64")
  expect_s3_class(out$treatment, "integer64")
  expect_s3_class(out$assigned_treatment, "integer64")
  # trial_period is always cast to Int32 by the core -> plain integer; outcome double.
  expect_type(out$trial_period, "integer")
  expect_type(out$outcome, "double")

  r_plain <- tters::expand_trial_df(base, estimand = "ITT")
  expect_structural_parity(
    out, r_plain,
    c("trial_period", "followup_time", "assigned_treatment", "treatment", "outcome")
  )
  expect_identical(as_dec(out$id), as_dec(bit64::as.integer64(r_plain$id) + offset64()))
})

# ---- Fitted path (raw cohort -> weighted frame in one call) with an integer64
#      id: the fit is id-shift-invariant, so the twin must agree on `weight`
#      (within the staged fitted tolerance) and the id must round-trip exactly. ----
test_that("expand_trial_weighted_fitted_df: integer64 id round-trips (high_switching PP)", {
  skip_if_no_bit64()
  skip_if_not_installed("arrow")
  skip_if(is.null(fx), "fixture battery not found (set TTERS_FIXTURE_DIR)")
  input <- file.path(fx, "scenarios", "input_high_switching.parquet")
  skip_if_not(file.exists(input), "high_switching fixture missing")

  cohort <- rd("scenarios/input_high_switching.parquet")
  cohort64 <- cohort
  cohort64$id <- bit64::as.integer64(cohort$id) + offset64()

  fit_args <- list(
    estimand = "PP", switch_numerator = "x2", switch_denominator = c("x2", "x1")
  )
  r_plain <- do.call(tters::expand_trial_weighted_fitted_df, c(list(cohort), fit_args))
  r_64 <- do.call(tters::expand_trial_weighted_fitted_df, c(list(cohort64), fit_args))

  expect_s3_class(r_64$id, "integer64")
  expect_identical(names(r_64), names(r_plain))
  expect_structural_parity(
    r_64, r_plain,
    c("trial_period", "followup_time", "assigned_treatment", "treatment", "outcome")
  )
  # id shifted exactly; weights agree within the staged fitted tolerance (1e-6,
  # mirror of weights_fit.rs::FITTED_WEIGHT_REL_TOL) — id shift cannot move them.
  expect_identical(as_dec(r_64$id), as_dec(bit64::as.integer64(r_plain$id) + offset64()))
  rel <- abs(r_64$weight - r_plain$weight) / pmax(abs(r_plain$weight), 1)
  expect_lte(max(rel), 1e-6)
})

# ---- The fitted factor table (fit_trial_weights_df) carries an integer64 id
#      back too. ----
test_that("fit_trial_weights_df: integer64 id is preserved in the factor table", {
  skip_if_no_bit64()
  skip_if_not_installed("arrow")
  skip_if(is.null(fx), "fixture battery not found (set TTERS_FIXTURE_DIR)")
  input <- file.path(fx, "scenarios", "input_high_switching.parquet")
  skip_if_not(file.exists(input), "high_switching fixture missing")

  cohort64 <- rd("scenarios/input_high_switching.parquet")
  cohort64$id <- bit64::as.integer64(cohort64$id) + offset64()

  ft <- tters::fit_trial_weights_df(
    cohort64,
    estimand = "PP", switch_numerator = "x2", switch_denominator = c("x2", "x1")
  )
  expect_identical(names(ft), c("id", "period", "weight_factor"))
  expect_s3_class(ft$id, "integer64")
  expect_type(ft$period, "integer")
  expect_type(ft$weight_factor, "double")
  expect_gt(nrow(ft), 0L)
  # Every factor-table id is one of the shifted cohort ids (>= 2^53 + 1).
  expect_true(all(ft$id >= bit64::as.integer64("9007199254740993")))
})
