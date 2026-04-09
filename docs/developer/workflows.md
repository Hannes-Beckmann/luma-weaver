# Workflows

This page summarizes the repository workflows that matter most to contributors and maintainers.

## CI

The main CI workflow is:

```text
.github/workflows/ci.yml
```

It currently does the following:

- validates add-on metadata
- runs Rust tests for `shared` and `backend`
- builds the frontend in two modes
  - standard frontend build
  - `demo-mode` frontend build for the GitHub Pages flavor
- runs a Docker smoke build on pushes to `main` and manual CI runs

The frontend build matrix is important because the GitHub Pages preview has its own wasm/demo build path and can regress independently from the normal backend-served frontend.

## Release Preflight

The repository includes:

```text
.github/workflows/release-preflight.yml
```

This workflow performs multi-arch container build checks without pushing images. It is useful when you want to validate release packaging before tagging a release.

## Add-on Publishing

The repository includes:

```text
.github/workflows/publish-addon.yml
```

This workflow:

- runs on version tags of the form `vX.Y.Z`
- validates that the tag version matches `config.yaml`
- builds and pushes per-architecture GHCR images
- creates a multi-arch manifest
- creates a GitHub release for tagged publishes

Current image target:

```text
ghcr.io/hannes-beckmann/luma-weaver-addon
```

## Pages Publishing

The repository includes:

```text
.github/workflows/publish-pages.yml
```

This workflow:

- runs manually with `workflow_dispatch`
- builds `crates/frontend` with `--features demo-mode`
- builds the docs site with `mdbook`
- builds generated Rust reference docs with `cargo doc --workspace --no-deps`
- assembles one GitHub Pages artifact containing the preview app, docs, and reference

Published paths:

- https://hannes-beckmann.github.io/luma-weaver/
- https://hannes-beckmann.github.io/luma-weaver/docs/
- https://hannes-beckmann.github.io/luma-weaver/reference/

## Local Verification Guidance

Use the smallest meaningful verification set for the files you touched.

Common commands:

- `cargo test -p shared -p backend --locked`
- `cargo test -p frontend --locked`
- `cargo build --locked --release -p backend`
- `trunk build --release`
- `trunk build --release --features demo-mode --public-url /luma-weaver/`
- `mdbook build docs-book`
- `cargo doc --workspace --no-deps`

## Branch And PR Conventions

Use the branch naming patterns documented in `AGENTS.md`:

- `feat/...`
- `fix/...`
- `ci/...`
- `docs/...`
- `chore/...`

Commit messages and PR titles should follow Conventional Commit style so changelog and release tooling remain predictable.

## Related Pages

- [contributing.md](contributing.md)
- [architecture.md](architecture.md)
- [protocol-runtime.md](protocol-runtime.md)
- [runtime-execution.md](runtime-execution.md)
