#!/bin/sh
# Build a SELF-CONTAINED, offline-installable source tarball of 'tters'.
#
# This is the local / source-distribution counterpart of the r-universe `.prepare`
# hook. r-universe has build-time network and just fetches the git dep; an offline
# or source install (e.g. a tarball handed to the upstream maintainers, or
# `R CMD INSTALL` with no network) needs the core crate AND every crates.io
# dependency bundled. This script produces exactly that, WITHOUT touching the
# committed working tree:
#
#   1. copy the package into a throwaway build dir (committed tree stays path-dep);
#   2. rewrite the path dep -> git+rev (default: a local bare clone via file://, so
#      `cargo vendor` needs NO network and the offline tarball is reproducible here;
#      pass a public URL as $1 to pin the canonical remote for a release artifact);
#   3. `cargo vendor` the whole tree (core + crates.io deps) -> vendor/ + the
#      source-replacement config (cargo BAKES the core's workspace inheritance,
#      preserving the exact Polars feature set => bit-for-bit determinism);
#   4. pack vendor.tar.xz + vendor-config.toml into src/rust/ (the existing Makevars
#      vendor plumbing consumes these and adds `--offline` in CRAN mode);
#   5. `R CMD build` -> a self-contained <pkg>_<ver>.tar.gz.
#
# The ~600 MB vendor tree is a RELEASE ARTIFACT: it is never committed (gitignored)
# and is regenerated from the pinned rev on demand / in CI.
#
# Usage: tools/build-offline-tarball.sh [git-url] [out-dir]
#   git-url : remote to pin (default: a fresh local bare clone, file://, offline)
#   out-dir : where to drop the tarball (default: $PWD)
set -eu

export PATH="$PATH:$HOME/.cargo/bin"

# Resolve repo root from this script's location (bindings/tters/tools/).
SCRIPT_DIR="$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)"
PKG_DIR="$(CDPATH='' cd -- "$SCRIPT_DIR/.." && pwd)"
REPO_ROOT="$(CDPATH='' cd -- "$PKG_DIR/../.." && pwd)"
REV="$(git -C "$REPO_ROOT" rev-parse HEAD)"
OUT_DIR="${2:-$PWD}"

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

# Git source: a local bare clone (offline) unless a URL is supplied.
if [ "${1:-}" = "" ]; then
    git clone --quiet --bare "$REPO_ROOT" "$WORK/rust-tte.git"
    URL="file://$WORK/rust-tte.git"
else
    URL="$1"
fi
echo "build-offline-tarball: core pinned to $URL @ $REV"

# 1) throwaway copy of the package (committed tree untouched).
cp -a "$PKG_DIR" "$WORK/tters"
rm -rf "$WORK/tters/src/rust/target" "$WORK/tters/src/rust/vendor" \
    "$WORK/tters/src/rust/.cargo" "$WORK/tters/src/.cargo" "$WORK/tters/src/vendor" \
    "$WORK/tters/src/Makevars" "$WORK/tters/src/tters.so" "$WORK/tters/src/"*.o

# 2) path dep -> git+rev (surgical, --locked-clean).
sh "$WORK/tters/tools/rewrite-core-dep.sh" "$URL" "$REV" "$WORK/tters/src/rust"

# 3) vendor the whole tree from src/ so layout matches the Makevars (./vendor,
#    config directory = "vendor"). --locked guarantees the exact pinned versions.
(cd "$WORK/tters/src" &&
    cargo vendor --locked --manifest-path rust/Cargo.toml vendor >rust/vendor-config.toml)

# 4) pack vendor.tar.xz (top-level vendor/) + keep vendor-config.toml; drop the
#    bulky unpacked tree so it never lands in the source tarball.
(cd "$WORK/tters/src" &&
    XZ_OPT=-1 tar -cJf rust/vendor.tar.xz --no-xattrs vendor &&
    rm -rf vendor)

# 5) build the source tarball.
(cd "$OUT_DIR" && R CMD build "$WORK/tters" --no-manual --no-build-vignettes)

VER="$(awk '/^Version:/{print $2}' "$PKG_DIR/DESCRIPTION")"
echo "build-offline-tarball: wrote $OUT_DIR/tters_${VER}.tar.gz (self-contained, offline)"
