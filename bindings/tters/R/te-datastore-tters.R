# The `te_datastore` companion backend.
#
# Wires the verified `tters` Rust + Polars expansion into upstream
# `TrialEmulation` so a user's `trial_sequence()` pipeline runs the expensive
# EXPANSION in Rust while estimation (sampling + MSM fit) stays in R. The Rust
# output is presented THROUGH `TrialEmulation`'s own `te_datastore` extension
# interface, so the downstream (`load_expanded_data()`, `sample_controls()`,
# `fit_msm()`) consumes it unchanged and bit-identically to the default path.
#
# `te_datastore` is a STORAGE seam: `expand_trials_trial_seq()` expands in R
# FIRST, then calls `save_expanded_data()`. So a backend that is merely
# registered would store already-R-expanded data and give no speedup. The value
# here is `expand_trials_tters()`, which runs the expansion in Rust and only then
# hands the result to the datastore.
#
# Everything that couples to `TrialEmulation` is gated behind
# `requireNamespace("TrialEmulation")`. `TrialEmulation` is a `Suggests`, never an
# `Imports`: plain `tters` (no `TrialEmulation`, no estimation) is unaffected, and
# the companion degrades gracefully when it is absent.

# `data.table` is a `Suggests` (pulled in transitively by `TrialEmulation`), not
# an `Imports`, so this namespace is not automatically "data.table aware". This
# flag is data.table's documented opt-in (see `vignette("datatable-importing")`)
# that lets `[.data.table` `:=`/`i`/`j` calls dispatch correctly from here. It is
# inert when data.table is not installed.
.datatable.aware <- TRUE

# data.table's non-standard evaluation (`:=` assignment, unquoted `setorder()`
# keys) reads as undefined globals to R CMD check's codetools scan; declare the
# two flagged symbols so the check stays clean. Purely a static-analysis hint —
# no runtime effect.
utils::globalVariables(c(":=", "id"))

# Session-local flag: has the S4 backend been registered yet? The
# `te_datastore_tters` class `contains = "te_datastore"`, whose parent lives in
# `TrialEmulation`, so the class cannot be defined at collation time when
# `TrialEmulation` is only a `Suggests`. It is defined the first time it is
# needed (and eagerly at load when `TrialEmulation` is already installed).
.tters_state <- new.env(parent = emptyenv())
.tters_state$te_backend_defined <- FALSE

#' Register the `te_datastore_tters` S4 backend (idempotent, internal)
#'
#' Defines the S4 class and its methods on `TrialEmulation`'s generics, but only
#' if both `TrialEmulation` and `data.table` are installed. Returns `TRUE` when
#' the backend is available afterwards, `FALSE` otherwise. Safe to call repeatedly.
#' @noRd
.tters_define_te_backend <- function() {
  if (isTRUE(.tters_state$te_backend_defined)) {
    return(invisible(TRUE))
  }
  if (!requireNamespace("TrialEmulation", quietly = TRUE) ||
    !requireNamespace("data.table", quietly = TRUE)) {
    return(invisible(FALSE))
  }

  # The generics `save_expanded_data` / `read_expanded_data` are owned by
  # `TrialEmulation`. When TrialEmulation is loaded-but-not-attached (the usual
  # state inside `.onLoad`), `setMethod("save_expanded_data", ...)` cannot resolve
  # the generic BY NAME ("no existing definition for function ..."), so we pass
  # the generic OBJECT explicitly via `::`. This also makes the load order of
  # `library(tters)` vs `library(TrialEmulation)` irrelevant.
  g_save <- TrialEmulation::save_expanded_data
  g_read <- TrialEmulation::read_expanded_data

  # The Rust-backed store. Mirrors `te_datastore_datatable`: it accumulates the
  # expanded `data.table` and tracks `@N` (inherited integer slot). Guarded so a
  # re-entry after a partial definition cannot hit a locked-class error.
  if (!methods::isClass("te_datastore_tters")) {
    methods::setClass(
      "te_datastore_tters",
      contains = "te_datastore",
      slots = c(data = "data.table")
    )
  }

  # save_expanded_data: APPEND a chunk and refresh @N. `expand_trials_tters()`
  # saves the whole expansion in one call, but this honours the multi-call
  # (id-chunked) contract too.
  methods::setMethod(
    g_save,
    "te_datastore_tters",
    function(object, data) {
      object@data <- rbind(object@data, data)
      object@N <- nrow(object@data)
      object
    }
  )

  # read_expanded_data: retrieve stored rows. Mirrors the data.table backend's
  # `trial_period %in% period` filter and `str2lang() + eval()` subset semantics
  # so cross-backend reads are identical.
  methods::setMethod(
    g_read,
    "te_datastore_tters",
    function(object, period, subset_condition) {
      trial_period <- NULL
      if (!is.null(period)) {
        if (!is.numeric(period) || anyNA(period) ||
          any(period < 0) || any(period != trunc(period))) {
          stop("`period` must be NULL or a vector of non-negative integers.", call. = FALSE)
        }
      }
      dt <- if (is.null(period)) {
        object@data
      } else {
        object@data[trial_period %in% period, ]
      }
      if (!is.null(subset_condition)) {
        dt <- dt[eval(str2lang(subset_condition))]
      }
      dt
    }
  )

  # NOTE: `sample_expanded_data` is deliberately NOT overridden. The base method
  # (dispatching on the `te_datastore` parent) snapshots/restores the RNG,
  # `set.seed(seed)`, calls our `read_expanded_data()`, then
  # `split(., trial_period x followup_time)` + `do_sampling`. Inheriting it makes
  # seeded sampling bit-identical to the data.table store, because our stored row
  # order matches `TrialEmulation`'s. Overriding would risk diverging the RNG.

  methods::setMethod(
    "show",
    "te_datastore_tters",
    function(object) {
      cat("A TE Datastore tters object (Rust + Polars expansion backend)\n")
      cat("N:", object@N, "observations\n")
      if (nrow(object@data) > 0L) print(object@data, nrows = 4L, topn = 2L)
    }
  )

  .tters_state$te_backend_defined <- TRUE
  invisible(TRUE)
}

#' @noRd
.tters_require_te_backend <- function(what) {
  if (isTRUE(.tters_define_te_backend())) {
    return(invisible(TRUE))
  }
  stop(sprintf(
    paste0(
      "%s requires the 'TrialEmulation' and 'data.table' packages. ",
      "Install them with install.packages(c('TrialEmulation', 'data.table'))."
    ),
    what
  ), call. = FALSE)
}

# Eagerly register the backend when TrialEmulation is already present, so the
# class is dispatch-ready before the first `save_to_tters()` call.
.onLoad <- function(libname, pkgname) {
  try(.tters_define_te_backend(), silent = TRUE)
}

#' Create a `te_datastore_tters` storage backend
#'
#' Constructor (the `save_to_*` convention) for the Rust-backed `te_datastore`
#' subclass. Like the reference backends it does no work — it returns an empty
#' store to hand to [TrialEmulation::set_expansion_options()]. The expansion is
#' run later by [expand_trials_tters()].
#'
#' Requires the `TrialEmulation` (and `data.table`) package: the returned object
#' is an S4 subclass of `TrialEmulation`'s `te_datastore`, so the class only
#' exists when `TrialEmulation` is installed.
#'
#' @return A `te_datastore_tters` object with `N = 0L` and an empty data slot.
#' @seealso [expand_trials_tters()] to populate it with a Rust-fast expansion.
#' @family save_to
#' @examples
#' \dontrun{
#' library(TrialEmulation)
#' trial_sequence("ITT") |>
#'   set_data(data = data_censored) |>
#'   set_outcome_model(adjustment_terms = ~x2) |>
#'   set_expansion_options(output = save_to_tters(), chunk_size = 0)
#' }
#' @export
save_to_tters <- function() {
  .tters_require_te_backend("save_to_tters()")
  methods::new("te_datastore_tters", data = data.table::data.table(), N = 0L)
}

#' Map a `trial_sequence` to the exact keeplist expanded frame, in Rust (internal)
#'
#' Runs the `tters` weighted-expansion `*_df` path on the trial sequence's cohort
#' and reshapes the result into the EXACT frame `TrialEmulation::expand()` would
#' produce for the `te_datastore` path: the `keeplist` columns, in `keeplist`
#' order, with the same dtypes and the same stored row order. Statistical
#' estimation stays in R — the per-period weight `wt` computed by
#' `calculate_weights()` is read verbatim and only its deterministic accumulation
#' happens in Rust.
#' @noRd
.tters_expanded_frame <- function(object) {
  trial_period <- followup_time <- period_new <- NULL

  cohort <- as.data.frame(object@data@data)

  # estimand drives the artificial-censoring state machine: PP censors each trial
  # at the first deviation, ITT does not. The flag is fixed by the estimand at
  # set_expansion_options() time (ITT -> FALSE, PP -> TRUE).
  estimand <- if (isTRUE(object@expansion@censor_at_switch)) "PP" else "ITT"

  # keeplist, exactly as expand_trials_trial_seq(): the six structural columns
  # then the outcome model's adjustment vars and treatment var.
  adj <- unique(object@outcome_model@adjustment_vars)
  treatment_var <- object@outcome_model@treatment_var
  keeplist <- unique(c(
    "id", "trial_period", "followup_time", "outcome", "weight", "treatment",
    adj, treatment_var
  ))

  # period window: clamp the requested range to the observed eligible range,
  # exactly as expand_trials_trial_seq() does before the chunk loop.
  elig <- cohort$eligible == 1
  if (!any(elig)) {
    stop("no eligible observations (eligible == 1) to expand.", call. = FALSE)
  }
  first_period <- max(c(object@expansion@first_period, min(cohort$period[elig])))
  last_period <- min(c(object@expansion@last_period, max(cohort$period[elig])))

  # per-(id, period) weight factor: the R-computed `wt` verbatim (defaulting to 1
  # when no weights were calculated, matching expand_trials_trial_seq()'s
  # `if (is.null(data$wt)) data[, wt := 1]`).
  wt <- if (is.null(cohort$wt)) rep(1, nrow(cohort)) else cohort$wt
  factors <- data.frame(id = cohort$id, period = cohort$period, weight_factor = wt)

  # the fast expansion + cumulative-weight accumulation, in Rust.
  expanded <- expand_trial_weighted_df(
    cohort = cohort,
    factors = factors,
    id_col = "id",
    period_col = "period",
    treatment_col = "treatment",
    eligible_col = "eligible",
    outcome_col = "outcome",
    first_period = first_period,
    last_period = last_period,
    estimand = estimand
  )
  dt <- data.table::as.data.table(expanded)

  # re-sort to TrialEmulation's te_datastore stored (generation / `index`) order:
  # (id, period_new = trial_period + followup_time, trial_period). tters' native
  # order is (id, trial_period, followup_time); the resort makes the stored frame
  # row-for-row identical to the default backend.
  dt[, period_new := trial_period + followup_time]
  data.table::setorder(dt, id, period_new, trial_period)
  dt[, period_new := NULL]

  # carry the baseline adjustment covariates: their value at the trial's start
  # period, i.e. the cohort row where period == trial_period, broadcast across
  # follow-up (a keyed lookup that preserves dt's row order).
  if (length(adj)) {
    base_cov <- data.table::as.data.table(cohort)[, c("id", "period", adj), with = FALSE]
    data.table::setkeyv(base_cov, c("id", "period"))
    joined <- base_cov[list(dt$id, dt$trial_period)]
    for (v in adj) dt[[v]] <- joined[[v]]
  }

  # exact keeplist column set + order.
  dt <- dt[, keeplist, with = FALSE]
  dt[]
}

#' @noRd
.tters_expand_trials_impl <- function(object) {
  if (!methods::is(object, "trial_sequence")) {
    stop(
      "`object` must be a 'trial_sequence' created by TrialEmulation::trial_sequence().",
      call. = FALSE
    )
  }
  if (methods::is(object, "trial_sequence_AT")) {
    stop(
      "the AT estimand (the computed `dose` column) is not yet supported by the tters backend.",
      call. = FALSE
    )
  }
  if (methods::is(object@expansion, "te_expansion_unset")) {
    stop("expansion options are not set; call set_expansion_options() first.", call. = FALSE)
  }
  if (methods::is(object@data, "te_data_unset")) {
    stop("no data is set; call set_data() first.", call. = FALSE)
  }

  mapped <- .tters_expanded_frame(object)
  object@expansion@datastore <-
    TrialEmulation::save_expanded_data(object@expansion@datastore, mapped)
  object
}

#' Expand a sequence of target trials with the Rust + Polars engine
#'
#' A drop-in replacement for [TrialEmulation::expand_trials()] that runs the
#' expensive expansion in Rust (`tters`) instead of R, then stores the result
#' through the `trial_sequence`'s registered `te_datastore`. The produced frame
#' is byte-equivalent to the default path (structural columns bit-exact, `weight`
#' to within machine precision), so the downstream — `load_expanded_data()`,
#' `sample_controls()`, `fit_msm()` — behaves identically.
#'
#' Estimation stays entirely in R. Weight *models* are fit by
#' `calculate_weights()`; this function reads that per-period `wt` verbatim and
#' Rust performs only the deterministic expansion and weight accumulation.
#'
#' Set up the `trial_sequence` exactly as for [TrialEmulation::expand_trials()]
#' (`set_data()` -> optional weight models + `calculate_weights()` ->
#' `set_outcome_model()` -> `set_expansion_options()`), then call this instead of
#' `expand_trials()`. The registered output may be [save_to_tters()] or any other
#' `te_datastore` (e.g. `save_to_datatable()`); the speedup comes from the Rust
#' expansion, not the store.
#'
#' @param object A configured `trial_sequence` (ITT or PP). The AT estimand is
#'   not yet supported and falls back to R.
#' @param fallback If `TRUE` (default), any failure of the Rust path (including an
#'   unsupported estimand or a missing toolchain) falls back to
#'   [TrialEmulation::expand_trials()] with a message. If `FALSE`, the error is
#'   raised.
#' @param quiet If `TRUE`, suppress the fallback message.
#' @return The updated `trial_sequence`, with its `@expansion@datastore`
#'   populated — the same object type [TrialEmulation::expand_trials()] returns.
#' @seealso [save_to_tters()]; [TrialEmulation::expand_trials()].
#' @examples
#' \dontrun{
#' library(TrialEmulation)
#' data("data_censored")
#' trial <- trial_sequence("ITT") |>
#'   set_data(data = data_censored) |>
#'   set_outcome_model(adjustment_terms = ~x2) |>
#'   set_expansion_options(output = save_to_tters(), chunk_size = 0)
#' trial <- expand_trials_tters(trial)
#' trial <- load_expanded_data(trial, seed = 1234, p_control = 0.5)
#' trial <- fit_msm(trial)
#' }
#' @export
expand_trials_tters <- function(object, fallback = TRUE, quiet = FALSE) {
  if (!requireNamespace("TrialEmulation", quietly = TRUE)) {
    stop(
      "`expand_trials_tters()` requires the 'TrialEmulation' package; install it to use the tters companion backend.",
      call. = FALSE
    )
  }
  .tters_require_te_backend("expand_trials_tters()")

  result <- tryCatch(.tters_expand_trials_impl(object), error = function(e) e)
  if (inherits(result, "error")) {
    if (!isTRUE(fallback)) stop(result)
    if (!isTRUE(quiet)) {
      message(
        "tters fast expansion unavailable (", conditionMessage(result),
        "); falling back to TrialEmulation::expand_trials()."
      )
    }
    return(TrialEmulation::expand_trials(object))
  }
  result
}
