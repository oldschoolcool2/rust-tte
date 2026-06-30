#!/bin/sh
# Synthesize the DISTRIBUTABLE form of the 'tte-expand' core dependency.
#
# The committed binding manifest keeps a PATH dependency on the sibling core crate
# (../../../../crates/tte-expand) so the monorepo dev workflow, the root `make
# verify`, and both lockfiles stay byte-identical and unchanged. That path escapes
# the package directory, so it cannot survive `R CMD build` (which tarballs only
# files under the package) nor `cargo vendor` (which never vendors a path dep).
#
# This script rewrites the path dep into a `git`+pinned-`rev` dependency so the
# subdir becomes self-contained for distribution. A git dep checks out the WHOLE
# repo, so the core crate's workspace inheritance (`polars = { workspace = true }`,
# edition, license, ...) resolves natively and the Polars feature set is identical
# BY CONSTRUCTION — preserving the engine's bit-for-bit determinism, unlike an
# in-package copy that would have to hand-transcribe (and could drift) the manifest.
#
# It edits, IN PLACE, the rust crate's Cargo.toml + Cargo.lock:
#   * Cargo.toml: the `tte-expand = { path = ... }` line -> `{ git = <url>, rev = <rev>, ... }`
#   * Cargo.lock: SURGICALLY inserts the matching `source = "git+<url>?rev=<rev>#<rev>"`
#     line into the existing `tte-expand` stanza, touching NOTHING else, so every
#     other pinned transitive version is preserved (a `--locked`-clean change, not a
#     full `cargo generate-lockfile` re-resolution which can drift a transitive).
#
# Run it against a THROWAWAY tree (an r-universe clone via .prepare, or a temp copy
# in tools/build-offline-tarball.sh) — never the committed working tree.
#
# Usage: rewrite-core-dep.sh <git-url> <rev> <path-to-src/rust>
set -eu

URL="$1"
REV="$2"
RUST_DIR="$3"

TOML="$RUST_DIR/Cargo.toml"
LOCK="$RUST_DIR/Cargo.lock"

[ -f "$TOML" ] || {
    echo "rewrite-core-dep: missing $TOML" >&2
    exit 1
}
[ -f "$LOCK" ] || {
    echo "rewrite-core-dep: missing $LOCK" >&2
    exit 1
}

# 1) Cargo.toml: path dep -> git+rev dep (preserve the weights-fit feature).
grep -q '^tte-expand = { path = ' "$TOML" || {
    echo "rewrite-core-dep: expected a 'tte-expand = { path = ... }' line in $TOML" >&2
    exit 1
}
sed "s|^tte-expand = { path = .*|tte-expand = { git = \"$URL\", rev = \"$REV\", features = [\"weights-fit\"] }|" \
    "$TOML" >"$TOML.tmp" && mv "$TOML.tmp" "$TOML"

# 2) Cargo.lock: surgically add the git source line to the tte-expand stanza only.
SRC="git+$URL?rev=$REV#$REV"
awk -v src="$SRC" '
  /^\[\[package\]\]/ { inpkg = 0 }
  /^name = "tte-expand"$/ { inpkg = 1 }
  { print }
  inpkg && /^version = / { print "source = \"" src "\""; inpkg = 0 }
' "$LOCK" >"$LOCK.tmp" && mv "$LOCK.tmp" "$LOCK"

echo "rewrite-core-dep: tte-expand -> git $URL @ $REV (in $RUST_DIR)"
