# Architecture

## Source of truth

- [`ROMUEQ_ARCHITECTURE_v2.0.md`](ROMUEQ_ARCHITECTURE_v2.0.md)
- [`ROMUEQ_SYSTEM_SCHEMA.md`](ROMUEQ_SYSTEM_SCHEMA.md)

## P0 gates

Production implementation is blocked until the following gates have evidence:

1. Parameter transport
2. Sample-accurate automation
3. Plugin framework
4. GUI/plugin embedding
5. Host lifecycle
6. Re-prepare, sample-rate and block-size behavior
7. Bypass behavior
8. Crossfade behavior
9. Determinism definition
10. Performance budget
11. Distribution and packaging
12. DAW compatibility

Each gate must produce a decision, evidence and an ADR.
