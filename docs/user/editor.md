# Editor Guide

This page describes the main frontend workflow in Luma Weaver.

## Dashboard

The dashboard is the graph-management view.

From here you can typically:

- create a graph
- open a graph in the editor
- rename a graph
- delete a graph
- inspect runtime state
- inspect graph-level diagnostics

The dashboard is also the safest place to orient yourself when you have multiple saved graphs.

## Editor

The editor opens one graph document at a time.

Inside the editor you can:

- add nodes
- connect nodes
- change parameter values
- inspect runtime values for debug-oriented nodes
- run the graph
- step or pause the runtime
- inspect diagnostics

## Common Editing Workflow

The usual graph-editing loop looks like this:

1. Open a graph from the dashboard.
2. Add nodes from the node menu.
3. Connect outputs to compatible inputs.
4. Adjust parameters and defaults.
5. Start the runtime and inspect the result.
6. Use diagnostics or debug-oriented nodes when behavior is not what you expect.

## Dashboard Actions

From the dashboard you can currently:

- create a graph document
- import a graph
- export a graph
- manage MQTT broker configs
- rename a graph
- remove a graph
- start, pause, or stop a graph without opening it
- open graph-level diagnostics

The `Open` action switches from the dashboard into the editor for the selected graph.

## Editor Header Actions

When a graph is open, the editor header gives you graph-level controls such as:

- run
- pause
- step by a configurable number of ticks
- stop
- export
- focus the canvas
- reload the selected graph from storage
- return to the dashboard
- undo and redo

These controls let you move between editing, runtime inspection, and recovery without leaving the graph.

## Imports And Exports

Graph import and export are available in backend mode.

Import behavior includes collision handling when a graph with the same id already exists. Export lets you save a graph document for sharing or backup.

The GitHub Pages demo is not intended to provide the full import/export and persistence story.

## Graph Lifecycle

Each graph has both an editable document and a runtime state.

Common lifecycle actions:

- `Open`: loads the graph into the editor
- `Start`: starts or resumes execution
- `Pause`: pauses execution while preserving runtime state
- `Step`: advances the graph a limited number of ticks
- `Stop`: stops the runtime

## Saving And Persistence

In normal use, graph changes are persisted on disk through the backend.

Persisted data includes:

- graph documents
- runtime resume state
- MQTT broker configuration

In demo mode, persistence is intentionally limited because the demo runs without a backend.

## Diagnostics

Diagnostics are shown both at graph level and node level.

They are useful when:

- a node is misconfigured
- a runtime compile step fails
- an integration is not available
- the current graph contains invalid or unsupported state

Use diagnostics to understand why a graph does not start cleanly or why a node is not behaving as expected.

## Runtime Inspection

Some nodes expose runtime-oriented previews or values, such as:

- `Plot`
- `Display`
- debug-oriented frame previews

These are especially useful when tuning math, signal, or animation graphs before sending output to a real integration target.
