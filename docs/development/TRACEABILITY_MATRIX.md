# Traceability Matrix

This matrix connects intent to release evidence. It is expanded as implementation tasks are completed.

| Requirement ID | Requirement | ADR | Component | Task(s) | Unit | Integration | Acceptance | Release gate |
|---|---|---|---|---|---|---|---|---|
| REQ-RT-001 | Audio hot path is allocation-free | ADR-007 | romueq-engine | M4-* | Required | Required | Allocation harness | M4/M10 |
| REQ-RT-002 | Audio hot path is non-blocking | ADR-007 | romueq-engine | M4-* | Required | Required | Stress/model test | M4/M10 |
| REQ-PARAM-001 | Stable parameter IDs | ADR-005 | romueq-common | M0/M3 | Required | Required | Schema test | M3/M11 |
| REQ-AUTO-001 | Preserve host event offsets | ADR-005 | romueq-engine/plugin | M2/M4/M7 | Required | Required | Automation sweep | M7/M9 |
| REQ-DSP-001 | Stable 8-band minimum-phase EQ | ADR-010 | romueq-dsp | M3 | Required | Required | Reference fixtures | M3/M10 |
| REQ-STATE-001 | Versioned state schema | ADR-006 | romueq-common/engine/plugin | M3/M7 | Required | Required | Migration fixtures | M7/M11 |
| REQ-BYPASS-001 | Defined bypass behavior | ADR-008 | romueq-engine/plugin | M4/M7 | Required | Required | Click/transient tests | M7/M9 |
| REQ-DET-001 | Deterministic documented behavior | ADR-009 | romueq-dsp/engine | M3/M4 | Required | Required | Repeated-render comparison | M10 |
| REQ-PLUGIN-001 | CLAP and VST3 MVP | ADR-002 | romueq-plugin | M7 | — | Required | Host matrix | M7/M9 |
| REQ-CLI-001 | Offline renderer matches engine | ADR-010 | romueq-cli | M8 | Required | Required | Golden render comparison | M8/M10 |
| REQ-REL-001 | Reproducible release artifacts | — | CI/release | M10/M11 | — | Required | Clean-machine install/smoke | M11 |

## Expansion rule

Every new P0/P1 requirement gets a stable `REQ-*` ID and must link to at least one task and one verification method before implementation begins.
