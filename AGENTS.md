# AGENTS.md — Equaform (RomuEq) Workspace & Domain Guide

Welcome to the **Equaform** workspace.

> **Global Engineering Standards Apply Automatically**:  
> All engineering methodology (strict TDD, 5-phase execution tracking, zero-panic/unwrap prohibition, mandatory merciless local audit via LM Studio, Headroom context compression, and atomic Git commit gates) are **enforced globally** by Antigravity rules (`idiomatic-craft`, `tiered-router`, `efficiency-router`).

---

## 1. Domain & Workspace Architecture

Before working on the codebase, review the domain-specific documentation:
1. `docs/agent/SESSION_STATE.md` — Current verified state of the workspace and audio safety status.
2. `docs/agent/HANDOFF.md` — Next tasks and context left by previous sessions.
3. `docs/agent/TASKS.md` — Milestone roadmap and task registry.
4. `docs/development/MASTER_PLAN.md` & `DEVELOPMENT_PLAN.md` — Architecture roadmap and milestones.
5. `docs/development/AI_DEVELOPMENT_PROTOCOL.md` — Project deterministic task execution guidelines.

---

## 2. Project-Specific Constraints (Equaform / RomuEq Only)

- **Language Mandate**: 100% English repository (code, comments, tests, commit messages, and docs).
- **Real-Time Safety (ADR-007 Inviolable)**:
  - **Zero Allocations on Audio Thread**: Never allocate (`Vec`, `Box`, `String`, etc.) in DSP rendering callbacks or audio hot paths. Pre-allocate all buffers, filter coefficients, and lookup tables at initialization.
  - **Zero Blocking & Zero I/O**: No mutex locks, channel synchronization, file I/O, or logging (`eprintln!`/`println!`) in the audio callback.
  - **Lock-Free Communication**: UI-to-audio parameter synchronization must exclusively use the lock-free triple buffer (`LockFreeTripleBuffer<EqParameters>`) or atomic primitives.
- **Cross-Platform DAW Plugin Targets**:
  - Standalone app and CoreAudio HAL driver are strictly internal/personal use.
  - Distribution targets are professional DAW plugins: **AUv2, VST3, CLAP** on macOS (Apple Silicon / Intel) and **VST3, CLAP** on Windows (DirectX 12 / Vulkan wgpu surface).
- **GUI Throttling & Energy Efficiency**:
  - Full 60 FPS redraw during mouse interaction and automation inspection.
  - Adaptive throttling to 15 FPS when inactive/unfocused, and 0 FPS dormancy when minimized/occluded.
