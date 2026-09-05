# ADR-005: Automation

**Status:** Accepted
**Date:** 2026-09-04

## Decision

RomuEq preserves host automation events with their block-relative sample offsets. The engine applies events in deterministic `(sample_offset, sequence)` order and never silently downgrades sample-accurate host automation to block-rate state.

GUI/control updates use the bounded SPSC control transport defined by ADR-001; host automation remains a separate event stream because hosts can deliver automation during processing with sample offsets.

## Rationale

CLAP exposes parameter events during `process()` and explicitly notes that sample-offset information is available there; VST3 delivers automation points through parameter queues sorted by sample offset. citeturn0search0turn0search3

## Consequences

- Adapters must preserve offsets exactly.
- Engine tests must cover multiple events at identical and different offsets.
- Any overflow or malformed-event behavior must be deterministic and documented.
