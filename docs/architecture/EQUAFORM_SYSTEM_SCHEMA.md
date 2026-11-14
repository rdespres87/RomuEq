# Equaform System Schema

## Canonical architecture diagrams

The Mermaid `.mmd` files in `diagrams/` are the source of truth for the system schema.

- `01-crate-dependencies.mmd` — Rust crate dependency graph
- `02-runtime-data-flow.mmd` — audio and control data flow
- `03-thread-model.mmd` — real-time vs non-real-time ownership
- `04-parameter-flow.mmd` — parameter and automation flow
- `05-plugin-boundaries.mmd` — CLAP/VST3 adapter boundaries
- `06-full-system.mmd` — complete architecture overview

Render example:

```bash
mmdc -i diagrams/06-full-system.mmd -o diagrams/06-full-system.svg
```

The audio thread must not depend on GUI, filesystem, network, blocking synchronization, or unbounded queues.
