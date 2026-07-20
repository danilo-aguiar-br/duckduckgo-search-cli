# NO CI / NO GitHub Actions

This repository **forbids** continuous integration on GitHub Actions,
Dependabot, pre-commit CI, and any remote publish pipeline.

## Forbidden

- `.github/workflows/**` (ci, release, audit, docs, etc.)
- Dependabot / Renovate configs that open automated PRs for CI
- `scripts/pre-publish-gate.sh` or any gate that calls `gh run list`
- Pre-commit hooks that require a remote runner
- Badges that claim green CI status

## Required local gates (maintainers)

Run before every tag and before `cargo publish`:

```bash
cargo check-all
cargo lint
cargo fmt --check
RUSTDOCFLAGS="-D warnings" cargo docs
cargo test-all          # or at least: cargo test --lib --all-features --locked
cargo deny check        # when deny.toml is present
cargo publish --dry-run --locked
```

Aliases live in [`.cargo/config.toml`](.cargo/config.toml).

## Release (manual only)

1. Bump `version` in `Cargo.toml` / `Cargo.lock` and update `CHANGELOG.md`.
2. Pass local gates above.
3. Commit on `main` (or merge a release branch into `main`).
4. Annotated tag: `git tag -a vX.Y.Z -m "Release vX.Y.Z: …"`.
5. Push: `git push origin main && git push origin vX.Y.Z`.
6. Optional GitHub Release notes via `gh release create` (no Actions).
7. Publish: `cargo publish --locked`.

There is **no** automatic crates.io upload on tag push.

## Optional local tooling

Host-only settings (mold/lld, sccache, `target-cpu=native`) belong in the
**user** `~/.cargo/config.toml`, never in this repository’s published config.
Do not bake host CPU features into crates.io artifacts.
