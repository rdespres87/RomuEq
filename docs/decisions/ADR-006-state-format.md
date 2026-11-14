# ADR-006: State format

**Status:** Accepted
**Date:** 2026-09-04

## Decision

Equaform MVP uses a versioned logical state schema with stable parameter IDs. The first schema version is `1` and contains the complete global and eight-band parameter state. State is logically distinct from presets, and DSP coefficients/filter history are not authoritative persisted state.

State save/load occurs outside the audio thread. Loading an unsupported future schema fails safely; supported migrations are explicit, deterministic, and tested.

## Rationale

Stable IDs and explicit migrations prevent project/preset corruption when the product evolves. The CLAP state extension defines state persistence for parameter and non-parameter state and specifies save/load on the main thread. citeturn0search1

## Consequences

- Parameter IDs, ranges and semantics must be treated as compatibility contracts.
- State migration tests are release-blocking.
- Plugin adapters map their native state representation to this logical schema.
