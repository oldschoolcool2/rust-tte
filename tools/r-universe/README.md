# r-universe registration for `tters`

This directory holds the canonical [r-universe](https://r-universe.dev) registry
entry for the `tters` R package, which lives in the `bindings/tters/` subdirectory
of this monorepo. r-universe is the realistic distribution channel for `tters`:
CRAN is **not** targeted because the vendored Polars dependency tree busts CRAN's
5 MB tarball limit (the same reason the `polars` R package left CRAN for
r-universe).

## Why this works (the build model)

r-universe build runners **clone the whole monorepo** and run a per-package
`.prepare` hook *while every sibling directory is still on disk*, then build the
subdir with `R CMD build`. They have **build-time network access** (ordinary
GitHub Actions runners; the cargo shim runs real `cargo` with no `--offline`).

So for r-universe we do **not** ship a vendored crate tree. Instead:

1. `bindings/tters/.prepare` rewrites the committed `tte-expand` **path** dependency
   into a `git`+pinned-`rev` dependency on this repo at the exact commit being
   built (`tools/rewrite-core-dep.sh`). The git rev is resolved dynamically from
   `git rev-parse HEAD`, so it always points at a commit that exists on the public
   repo.
2. `R CMD build` then tarballs the self-contained subdir.
3. The downstream `cargo build` fetches the core crate + crates.io dependencies
   over the network. Because the dep is a **git** dep (not a copy), cargo resolves
   the core's workspace inheritance (`polars = { workspace = true }`, edition, …)
   natively — the Polars feature set is identical by construction, preserving the
   engine's bit-for-bit determinism.

The committed working tree is untouched (it keeps the path dep), so the monorepo
dev workflow and `make verify` are unaffected.

## Go-live steps (owner action — public, outward-facing)

These are deliberately **not** automated here; they are public, account-level
GitHub actions for the repository owner to perform (or to explicitly approve):

1. **Make the repository public** (r-universe only builds public repos):

   ```sh
   gh repo edit oldschoolcool2/rust-tte --visibility public --accept-visibility-change-consequences
   ```

   The repo has been gitleaks-guarded since Phase 0; run `/check-secrets` (or
   `gitleaks detect`) once more before flipping if in any doubt.

2. **Create the registry repository** `oldschoolcool2.r-universe.dev` on GitHub and
   push `packages.json` (the file next to this README) to its root:

   ```sh
   gh repo create oldschoolcool2.r-universe.dev --public
   # then commit tools/r-universe/packages.json as packages.json at that repo root
   ```

3. r-universe picks it up automatically and builds `tters` from
   `bindings/tters/` on every push to this repo's default branch. The built
   package appears at `https://oldschoolcool2.r-universe.dev/tters` and installs
   with:

   ```r
   install.packages("tters", repos = "https://oldschoolcool2.r-universe.dev")
   ```

## Offline / source tarball (independent of r-universe)

For a fully self-contained tarball (e.g. to hand to the upstream maintainers, or
for an offline install), run:

```sh
sh bindings/tters/tools/build-offline-tarball.sh            # local, offline (file:// core)
sh bindings/tters/tools/build-offline-tarball.sh https://github.com/oldschoolcool2/rust-tte  # pin the public remote
```

This vendors the core + all crates.io deps into `src/rust/vendor.tar.xz` (a
~600 MB tree, never committed — see `.gitignore`) and produces a
`tters_<ver>.tar.gz` that installs with `R CMD INSTALL` and no network.
