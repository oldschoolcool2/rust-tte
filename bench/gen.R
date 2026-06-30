# bench/gen.R — FAST, VECTORIZED, SEEDED generator of well-formed TTE input parquet.
#
# Produces the same input schema the engine's scenario fixtures use:
#   id(int32) period(int32 0..T) eligible(int32) treatment(int32) x1(double) x2(int32) outcome(int32)
# It reproduces the semantics of oracle/10_simulate.R in pure vectorized form (no
# per-row loop): absorbing treatment from a per-patient initiation period, a slice
# that never initiates (max fan-out), a rare outcome that truncates each patient's
# rows. Fully DETERMINISTIC in (n_patients, periods, seed): the same arguments
# always emit a byte-identical parquet (no wall-clock / env reads). This is the
# benchmark counterpart to the immutable Oracle fixtures.
#
# Usage: Rscript bench/gen.R <n_patients> <periods_per_patient> <seed> <out_parquet> [haz] [p_out] [p_never]
suppressPackageStartupMessages({ library(data.table); library(arrow) })

args  <- commandArgs(trailingOnly = TRUE)
n     <- as.integer(args[[1]]); P <- as.integer(args[[2]])
seed  <- as.integer(args[[3]]); out <- args[[4]]
haz   <- if (length(args) >= 5) as.numeric(args[[5]]) else 0.22
p_out <- if (length(args) >= 6) as.numeric(args[[6]]) else 0.006
p_nev <- if (length(args) >= 7) as.numeric(args[[7]]) else 0.10

t0 <- proc.time()[["elapsed"]]; set.seed(seed)

# per-patient ABSORBING initiation period; a slice never initiate -> seed P trials
init <- rgeom(n, haz); init[runif(n) < p_nev] <- P

# full contiguous panel (one vectorized allocation, no row loop)
dt <- data.table(id = rep.int(seq_len(n), rep.int(P, n)),
                 period = rep.int(0:(P - 1L), n))
dt[, init := rep.int(init, rep.int(P, n))]
dt[, `:=`(eligible  = as.integer(period <= init),   # naive at start of period
          treatment = as.integer(period >= init),
          x1        = rnorm(.N),
          x2        = as.integer(runif(.N) < 0.5),
          outcome   = as.integer(runif(.N) < p_out))]

# truncate each patient at its FIRST outcome (matches the simulator's break)
dt[, firstY := { w <- which(outcome == 1L); if (length(w)) period[w[1L]] else .Machine$integer.max }, by = id]
dt <- dt[period <= firstY]; dt[, c("init", "firstY") := NULL]

dt[, `:=`(id = as.integer(id), period = as.integer(period), eligible = as.integer(eligible),
          treatment = as.integer(treatment), x1 = as.double(x1), x2 = as.integer(x2),
          outcome = as.integer(outcome))]
setcolorder(dt, c("id", "period", "eligible", "treatment", "x1", "x2", "outcome"))
setkey(dt, id, period)

stopifnot(all(dt$eligible %in% c(0L, 1L)), all(dt$treatment %in% c(0L, 1L)),
          all(dt$outcome %in% c(0L, 1L)),
          dt[period == 0L, all(eligible == 1L)],
          dt[, all(diff(period) == 1L), by = id][, all(V1)],
          dt[, period[1L] == 0L, by = id][, all(V1)])

arrow::write_parquet(dt, out, compression = "snappy")
t1 <- proc.time()[["elapsed"]]; fi <- file.info(out)
cat(sprintf("GEN n_patients=%d P=%d seed=%d -> input_rows=%d gen_wall_s=%.2f parquet_MB=%.1f mean_periods=%.2f\n",
            n, P, seed, nrow(dt), t1 - t0, fi$size / 1048576, nrow(dt) / n))
