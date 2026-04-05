# Contributing

This page describes the working conventions for contributors and coding agents in `luma-weaver`.

## Source Of Truth

Use these files together:

- [../../README.md](../../README.md): project overview, development commands, and deployment notes
- [../../AGENTS.md](../../AGENTS.md): repo-specific execution guidance and maintenance checklists
- [architecture.md](architecture.md): workspace and runtime architecture
- [node-authoring.md](node-authoring.md): node implementation workflow

## Git Workflow

Follow the repo conventions documented in `AGENTS.md`:

- branch naming: `feat/...`, `fix/...`, `ci/...`, `chore/...`, `docs/...`
- commit subjects: Conventional Commit style
- PR descriptions: include summary, why, testing, and compatibility notes when relevant

When working from GitHub issues:

- check the issue body and comments for dependency notes before starting
- keep dependent issues sequenced in the requested order unless explicitly told otherwise
- prefer one branch and one PR per independent issue

## Verification Expectations

Match verification to the code you touch:

- shared model/protocol: `cargo test -p shared -p backend --locked`
- backend/runtime/integrations: `cargo test -p backend --locked`
- frontend/editor/UI: `cargo test -p frontend --locked` and `trunk build --release` from `crates/frontend`
- workflow/Docker/add-on changes: verify the matching workflow files and configuration paths stay aligned

If a relevant check could not be run, call that out explicitly in the PR.

## Scope Discipline

- keep changes narrowly scoped to the issue or request
- avoid mixing refactors with feature work unless the refactor is required
- when a change affects persisted graphs, diagnostics, or deployment behavior, document that clearly

## Docs Discipline

Update docs when the user-facing or contributor-facing behavior changes.

Common docs touchpoints:

- [../../README.md](../../README.md)
- [../../DOCS.md](../../DOCS.md)
- [architecture.md](architecture.md)
- [node-authoring.md](node-authoring.md)

## Dependency And Compatibility Risk

Treat these as high-risk areas that deserve extra care:

- persisted graph compatibility
- unknown node handling and schema drift
- diagnostics behavior across dashboard/editor
- asset storage, portability, import/export, and graph statistics
