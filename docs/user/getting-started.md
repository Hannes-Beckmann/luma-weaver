# Getting Started

This page is the first-use walkthrough for people who want to use Luma Weaver rather than work on the codebase.

It is intentionally lightweight:

- use this page to understand what Luma Weaver is and what to do first
- use [deployment.md](deployment.md) to choose how to run it
- use [editor.md](editor.md) once you are inside the app

## Choose A Starting Point

Most users should think about Luma Weaver in terms of deployment style rather than runtime mode.

The main options are:

- standalone backend deployment with Docker or Docker Compose
- Home Assistant add-on deployment
- the GitHub Pages demo when you only want a quick preview

## First Run

The quickest standalone path is:

```bash
docker compose up --build
```

Then open:

```text
http://localhost:38123/
```

If you are using the Home Assistant add-on, start the add-on and open the web UI from Home Assistant or directly on port `38123`.

## Core Concepts

Luma Weaver is built around graph documents.

Each graph contains:

- nodes
- edges between node outputs and inputs
- per-node parameters
- an execution frequency
- editor viewport state

At a high level:

- the dashboard shows the graphs available in storage
- the editor opens one graph at a time
- the runtime can start, pause, step, or stop a graph

## Typical First Workflow

1. Open the app.
2. Create a new graph or open an existing one.
3. Add a few generator, math, frame, or output nodes.
4. Connect outputs to inputs.
5. Adjust parameters.
6. Start the graph and inspect runtime output or diagnostics.

## Next Pages

- [deployment.md](deployment.md): where and how to run Luma Weaver
- [editor.md](editor.md): dashboard and editor workflow
- [demo-mode.md](demo-mode.md): GitHub Pages demo notes
- [integrations.md](integrations.md): WLED and Home Assistant MQTT
- [troubleshooting.md](troubleshooting.md): common setup and runtime issues
