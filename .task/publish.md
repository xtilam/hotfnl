# Publish Task — hotfnl

## Context
- 2 packages: `hotfnl-proc-macro` (publish first) and `hotfnl` (publish second, depends on the proc lib)
- Current version: Cargo.toml is `0.1.8`
- Latest git tag: `v0.1.7` (no v0.1.8 tag yet)

## Step 1 — Check proc-macro changes
- Compare the code in `packages/hotfnl-proc-macro/` against the previously published version.
- Decide whether to bump `hotfnl-proc-macro`:
  - Changed → bump both packages to the new version.
  - Unchanged → only bump `hotfnl`, keep the proc-macro as is.

## Step 2 — Bump version
- If the proc-macro changed: bump the version in the Cargo.toml of `hotfnl-proc-macro` AND `hotfnl` to the same value (e.g. 0.1.8).
- Otherwise: only bump `hotfnl` (e.g. 0.1.8).

## Step 3 — Git commit + tag
1. Commit the code.
2. Delete the old tag locally: `git tag -d v0.1.7` (ignore errors).
3. Push the tag deletion: `git push origin :refs/tags/v0.1.7` (ignore errors).
4. Create the new tag: `git tag v0.1.8`.
5. Push the tag: `git push origin v0.1.8`.

## Step 4 — Publish to crates.io
1. `cargo publish` for `hotfnl-proc-macro` (if bumped).
2. `cargo publish` for `hotfnl`.
