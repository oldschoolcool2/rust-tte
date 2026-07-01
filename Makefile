# tte-expand — Phase-5 reproducibility + benchmark entrypoints.
#
#   make verify       Computational-reproducibility certificate (pure cargo): run
#                     the contract suite, then regenerate report/certificate.md by
#                     recomputing every fixture SHA-256 and re-verifying equivalence.
#   make bench        Full criterion runtime sweep of the engine (local).
#   make bench-smoke  Fast CI smoke (input rows <= 1e5, criterion --quick).
#   make curves       R-vs-Rust runtime + peak-RSS curves incl. the R-OOM regime
#                     (needs R + TrialEmulation + GNU /usr/bin/time; heavy, manual).
#   make golden       Tier-2 whole-pipeline golden: Rust expands, R estimates, the
#                     coefficients match upstream initiators() within tolerance
#                     (needs R + TrialEmulation; manual).
#
# `verify` and `bench-smoke` are the CI surface; `curves` and `golden` are manual.

CARGO ?= cargo
export PATH := $(HOME)/.cargo/bin:$(PATH)

.PHONY: help verify test certificate bench bench-smoke curves golden clones antipatterns clean

help:
	@printf 'targets: verify | test | certificate | bench | bench-smoke | curves | golden | clones | antipatterns | clean\n'

verify: test certificate

test:
	$(CARGO) test --workspace --all-features --all-targets --locked
	$(CARGO) test --workspace --all-features --doc --locked

certificate:
	$(CARGO) test --bench certificate --features weights-fit --locked

bench:
	$(CARGO) bench --bench expand

bench-smoke:
	TTE_BENCH_MAX_ROWS=100000 $(CARGO) bench --bench expand -- --quick

curves:
	bash bench/run_bench.sh

golden:
	bash bench/run_golden.sh

# Copy-paste (clone) detection over the editable Rust + R sources (config in
# .jscpd.json; needs Node/npx). The 8%-duplicated-lines threshold accepts the
# deliberate residue — flat FFI shim signatures and argument forwarding in the
# tters binding, the per-dtype marshalling arms in frame.rs, and the explicit
# argument pass-through of the R user-facing wrappers — while failing on new
# copy-paste. Generated files and the immutable tests/ are excluded.
clones:
	npx --yes jscpd

# Rust-focused agent-antipattern scan (needs uv; rules in
# .claude/semgrep/agent-antipatterns.yaml): path-scoped determinism bans
# (wall-clock / env reads / RNG / hash-order in the transform path) that
# clippy cannot express, plus the Rust Design Patterns book's
# deny(warnings) and borrow-checker-clone anti-patterns. --error fails
# the target on any finding.
antipatterns:
	uvx semgrep --config .claude/semgrep/agent-antipatterns.yaml \
		crates bindings/tters/src/rust/src bench --metrics=off --error

clean:
	$(CARGO) clean
	rm -rf target/criterion
