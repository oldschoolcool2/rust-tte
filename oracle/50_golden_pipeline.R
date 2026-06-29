# oracle/50_golden_pipeline.R
source("oracle/00_setup.R")

golden_pipeline <- function(data, name, out_dir = file.path(OUT_ROOT, "golden")) {
  dir.create(out_dir, recursive = TRUE, showWarnings = FALSE)
  res <- initiators(
    data = data, id = "id", period = "period", treatment = "treatment",
    outcome = "outcome", eligible = "eligible",
    estimand_type = "ITT", model_var = "assigned_treatment",
    outcome_cov = ~1, use_censor_weights = FALSE, quiet = TRUE
  )
  # VERIFY accessor on your version: summary(res) prints a robust coef table with
  # columns names/estimate/robust_se/2.5%/97.5%/z/p_value. Grab the data behind it.
  robust_tab <- tryCatch(res$robust$summary, error = function(e) NULL)
  if (is.null(robust_tab)) robust_tab <- summary(res)$coefficients  # fallback
  jsonlite::write_json(
    list(provenance = ORACLE_PROVENANCE, dataset = name,
         tolerance = list(log_or = 1e-4, robust_se = 1e-3),  # documented contract
         coefficients = robust_tab),
    file.path(out_dir, sprintf("golden_%s_itt.json", name)),
    pretty = TRUE, digits = 12, auto_unbox = TRUE
  )
  message("Golden -> ", name)
}
