# GitHub Pages Preview

This page describes the browser-only preview build that can be published to GitHub Pages.

This is a supporting page, not the main deployment guide.

- For normal deployment choices, start with [deployment.md](deployment.md).
- Use this page when you specifically need the hosted preview behavior.

## What The Preview Is

The preview is a static frontend build with the `demo-mode` feature enabled.

Instead of connecting to the backend over WebSocket, the frontend uses an in-browser transport and a portable runtime. That makes it possible to host a working editor demo on GitHub Pages.

Live demo:

- https://hannes-beckmann.github.io/luma-weaver/

## What It Is Good For

The preview is useful for:

- sharing the editor quickly
- giving new users a zero-setup preview
- demonstrating graph editing concepts
- testing portable graph behavior in the browser

## What It Does Not Include

The preview is intentionally not full backend mode.

Backend-dependent features are limited or unavailable, including:

- real graph persistence on the backend
- WLED discovery and output
- MQTT broker management and Home Assistant integration
- backend-hosted services and endpoints

If you need those features, run the normal backend-served application.

## URL Behavior On GitHub Pages

The Pages demo is designed to live under the repository base path, for example:

```text
/luma-weaver/
```

Deep links like `/luma-weaver/graphs/<graph-id>` are supported by redirecting GitHub Pages 404 requests back through the app entry page and then restoring the original route before the frontend starts.

## Publishing

The repository includes a manual workflow:

```text
.github/workflows/publish-pages.yml
```

That workflow builds:

```text
trunk build --release --features demo-mode --public-url /luma-weaver/
```

and publishes the resulting static bundle to GitHub Pages.

## When To Use Backend Mode Instead

Prefer backend mode when you need:

- persistent saved work
- network discovery
- full runtime integration behavior
- the same environment used for standalone Docker or Home Assistant
