# Contributing

This page describes the working conventions for contributors and coding agents in `luma-weaver`.

Use this page as the developer entry point.

- If you want to understand the codebase shape, start with [architecture.md](architecture.md).
- If you want to understand backend service ownership, read [backend-objects.md](backend-objects.md).
- If you want to change protocol, subscriptions, or WebSocket behavior, read [protocol-runtime.md](protocol-runtime.md).
- If you want to change graph compilation or runtime execution, read [runtime-execution.md](runtime-execution.md).
- If you want to add or change built-in nodes, read [node-authoring.md](node-authoring.md).
- If you want to understand CI, releases, or publishing, read [workflows.md](workflows.md).

## Source Of Truth

Use these files together:

- [README.md](https://github.com/Hannes-Beckmann/luma-weaver/blob/main/README.md): project overview, development commands, and deployment notes
- [AGENTS.md](https://github.com/Hannes-Beckmann/luma-weaver/blob/main/AGENTS.md): repo-specific execution guidance and maintenance checklists
- [architecture.md](architecture.md): workspace and runtime architecture
- [backend-objects.md](backend-objects.md): long-lived backend objects and their relationships
- [protocol-runtime.md](protocol-runtime.md): protocol and WebSocket internals
- [runtime-execution.md](runtime-execution.md): compilation, render contexts, and tick execution
- [node-authoring.md](node-authoring.md): node implementation workflow

## When To Read Which Page

- `architecture.md`: crate boundaries, module responsibilities, deployment-level structure
- `backend-objects.md`: long-lived backend service instances, ownership, and process-wide relationships
- `protocol-runtime.md`: frontend/backend messaging, subscriptions, and transport routing
- `runtime-execution.md`: graph compilation, render contexts, tick execution, runtime manager behavior
- `node-authoring.md`: built-in node lifecycle across shared schema, backend runtime, registry, and editor
- `workflows.md`: CI, Docker/release validation, add-on publishing, and Pages publishing

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

- [README.md](https://github.com/Hannes-Beckmann/luma-weaver/blob/main/README.md)
- [DOCS.md](https://github.com/Hannes-Beckmann/luma-weaver/blob/main/DOCS.md)
- [architecture.md](architecture.md)
- [node-authoring.md](node-authoring.md)

Choose the deeper developer page by topic:

- system/module shape: [architecture.md](architecture.md)
- service ownership: [backend-objects.md](backend-objects.md)
- message flow: [protocol-runtime.md](protocol-runtime.md)
- runtime behavior: [runtime-execution.md](runtime-execution.md)
- node lifecycle: [node-authoring.md](node-authoring.md)
- repo automation and publishing: [workflows.md](workflows.md)

## Dependency And Compatibility Risk

Treat these as high-risk areas that deserve extra care:

- persisted graph compatibility
- unknown node handling and schema drift
- diagnostics behavior across dashboard/editor
- asset storage, portability, import/export, and graph statistics
