# ADR-001: Parameter transport

Status: Proposed

## Context

The audio thread cannot safely consume mutable GUI state directly.

## Decision

Use a bounded, preallocated producer/consumer command path for control-thread updates. Host automation events remain a distinct event stream and preserve their block-relative offsets.

## Evidence required

- concurrency model checking;
- queue stress tests;
- overflow policy tests;
- allocation checks.

## Open question

Freeze the exact queue implementation only after the prototype passes.
