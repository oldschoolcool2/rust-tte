# bench/golden_roundtrip.R — Tier-2 whole-pipeline golden (ZERO statistics in Rust).
#
# Closes the reproducibility loop without porting any solver: the Rust engine
# expands `data_censored` (ITT) to Parquet; R reads it back and runs `trial_msm()`
# (the pooled-logistic glm + robust SE); the resulting coefficients must equal
# upstream `initiators(data_censored)` within the documented tolerance
# (log-OR 1e-4, robust SE 1e-3). The glm / sandwich estimation stays entirely in R.
# Writes report/golden.md and exits non-zero on mismatch.
#
# Invoked by bench/run_golden.sh:  Rscript bench/golden_roundtrip.R <runner_bin> <repo_root>
suppressPackageStartupMessages({ library(arrow); library(TrialEmulation); library(data.table) })

args   <- commandArgs(trailingOnly = TRUE)
runner <- args[[1]]
root   <- args[[2]]
tol_est <- 1e-4   # log-odds-ratio
tol_se  <- 1e-3   # robust standard error

tmp <- tempfile("golden_"); dir.create(tmp)
raw <- file.path(tmp, "raw.parquet")
exp <- file.path(tmp, "expanded.parquet")

data(data_censored)
arrow::write_parquet(as.data.frame(data_censored), raw)

# 1. Rust ITT expansion (the verified, deterministic core).
out <- system2(runner, c(raw, exp, "itt"), stdout = TRUE, stderr = TRUE)
if (!file.exists(exp)) stop("Rust runner did not produce the expanded frame:\n", paste(out, collapse = "\n"))

rust_exp <- as.data.frame(arrow::read_parquet(exp))
rust_exp$weight <- 1.0   # unweighted ITT MSM (IPCW disabled) => unit weights

# 2. R estimates on the RUST-expanded frame (no Rust statistics involved).
msm <- trial_msm(data = rust_exp, outcome_cov = ~1, model_var = "assigned_treatment", quiet = TRUE)
rust_coef <- msm$robust$summary

# 3. Upstream reference: initiators() runs the SAME expansion + estimation in R.
up <- initiators(
  data = data_censored, id = "id", period = "period", treatment = "treatment",
  outcome = "outcome", eligible = "eligible", estimand_type = "ITT",
  model_var = "assigned_treatment", outcome_cov = ~1, use_censor_weights = FALSE, quiet = TRUE
)
up_coef <- up$robust$summary

# 4. Compare within tolerance.
m <- merge(
  rust_coef[, c("names", "estimate", "robust_se")],
  up_coef[,   c("names", "estimate", "robust_se")],
  by = "names", suffixes = c("_rust", "_up")
)
m$d_est <- abs(m$estimate_rust - m$estimate_up)
m$d_se  <- abs(m$robust_se_rust - m$robust_se_up)
d_est <- max(m$d_est); d_se <- max(m$d_se)
pass  <- is.finite(d_est) && is.finite(d_se) && d_est <= tol_est && d_se <= tol_se

# 5. Write report/golden.md.
lines <- c(
  "# Tier-2 Whole-Pipeline Golden — Rust-expand -> R-estimate",
  "",
  sprintf("**Verdict:** %s", if (pass) "PASS" else "FAIL"),
  "",
  "The Rust engine expands `data_censored` (ITT); R's `trial_msm()` estimates on",
  "that frame; the coefficients are compared to upstream `initiators()`. No glm /",
  "sandwich code runs in Rust. Tolerance: log-OR 1e-4, robust SE 1e-3.",
  "",
  sprintf("- max |Δ estimate| = %.3e (tol %.0e)", d_est, tol_est),
  sprintf("- max |Δ robust_se| = %.3e (tol %.0e)", d_se, tol_se),
  "",
  "| term | est (Rust→R) | est (upstream) | Δest | rse (Rust→R) | rse (upstream) | Δrse |",
  "|---|---:|---:|---:|---:|---:|---:|"
)
for (i in seq_len(nrow(m))) {
  lines <- c(lines, sprintf("| `%s` | %.8f | %.8f | %.2e | %.8f | %.8f | %.2e |",
    m$names[i], m$estimate_rust[i], m$estimate_up[i], m$d_est[i],
    m$robust_se_rust[i], m$robust_se_up[i], m$d_se[i]))
}
writeLines(lines, file.path(root, "report", "golden.md"))
cat(sprintf("golden: %s (max dEst=%.2e, max dSE=%.2e) -> report/golden.md\n",
            if (pass) "PASS" else "FAIL", d_est, d_se))
quit(status = if (pass) 0L else 1L)
