# oracle/10_simulate.R — longitudinal cohort simulator for sequential TTE
source("oracle/00_setup.R")

# Returns one person-period long data.frame in INPUT_COLS schema.
# Epidemiology baked in:
#  - x1 is a time-varying confounder (AR(1)) affecting BOTH initiation and outcome
#    => genuine time-varying confounding, the thing IPCW exists to handle.
#  - eligibility = treatment-naive at start of period (recurrent until initiation)
#    => one patient legitimately seeds MULTIPLE trial_periods (the core behaviour).
#  - switch_prob > 0 lets treated patients deviate => exercises PP artificial censoring.
#  - a person's rows stop at first of outcome / censoring / max_period.
simulate_cohort <- function(n, max_period, params, seed) {
  set.seed(seed)
  p <- modifyList(list(
    L_ar = 0.8, L_sd = 0.7, x2_int = -0.2,     # confounder dynamics
    init_int = -2.0, conf_AL = 0.8,            # treatment initiation (hazard, confounded by L)
    out_int  = -3.0, beta_A = -0.5, conf_YL = 0.7,  # outcome hazard
    cens_prob = 0.02, switch_prob = 0.0        # censoring; switch_prob=0 => absorbing treatment
  ), params)

  out_list <- vector("list", n)
  for (i in seq_len(n)) {
    L <- rnorm(1)
    A_prev <- 0L
    rows_i <- vector("list", max_period + 1L)
    k <- 0L
    for (t in 0:max_period) {
      L  <- p$L_ar * L + rnorm(1, 0, p$L_sd)
      x2 <- rbinom(1, 1, plogis(p$x2_int + 0.5 * L))
      eligible <- as.integer(A_prev == 0L)            # naive => eligible
      if (A_prev == 1L) {
        A <- if (runif(1) < p$switch_prob) 0L else 1L # may deviate (PP stress)
      } else {
        A <- rbinom(1, 1, plogis(p$init_int + p$conf_AL * L))
      }
      Y <- rbinom(1, 1, plogis(p$out_int + p$beta_A * A + p$conf_YL * L))
      C <- rbinom(1, 1, p$cens_prob)
      k <- k + 1L
      rows_i[[k]] <- data.frame(
        id = i, period = t, eligible = eligible,
        treatment = A, x1 = L, x2 = x2, outcome = Y
      )
      A_prev <- A
      if (Y == 1L || C == 1L) break
    }
    out_list[[i]] <- do.call(rbind, rows_i[seq_len(k)])
  }
  df <- do.call(rbind, out_list)
  rownames(df) <- NULL
  df[, INPUT_COLS]
}

# DGP self-check: assert the simulator produces well-formed input before it ever
# reaches the Oracle. Catches a broken generator early (cheaper than a failed dump).
validate_input <- function(df) {
  stopifnot(
    all(INPUT_COLS %in% names(df)),
    all(df$eligible %in% c(0L, 1L)),
    all(df$treatment %in% c(0L, 1L)),
    all(df$outcome %in% c(0L, 1L)),
    # per id, periods are contiguous from their first value
    df[, all(diff(sort(unique(period))) == 1L), by = id][, all(V1)]
  )
  invisible(df)
}
