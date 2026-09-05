# RomuEq — M0 Decision Matrix

**Date:** 2026-09-03  
**Purpose:** one-page decision sheet for closing the P0 decisions required by M0.  
**Rule:** this matrix records recommendations; it does **not** silently approve any unresolved P0 decision.

| DR | Decision to make | Current status | Recommended decision | What is required to mark PASS? | Owner |
|---|---|---|---|---|---|
| **DR-2026-001** | Can `RomuEq` remain the public product/brand name? | **OPEN** | Keep `RomuEq` as working name, but **do not publicly release under it** until clearance is accepted. | Exact + similarity trademark search; target-jurisdiction review; domain/social checks; residual-risk assessment; explicit acceptance or rename decision. | Product owner + legal/IP reviewer |
| **DR-2026-002** | How are control updates transported to the engine? | **OPEN** | Bounded, preallocated **SPSC control-command queue** for GUI/control updates + separate host automation event stream preserving block-relative sample offsets. | Prototype; stress test; overflow policy; real-time safety evidence; deterministic tests; explicit ADR acceptance. | Architecture/DSP owner |
| **DR-2026-003** | Which plugin adapter strategy is frozen? | **OPEN** | Prefer **isolated CLAP/VST3 adapters** and avoid freezing the core around an unverified third-party framework. | Maintenance/license/ABI review; automation/state/GUI tests; hostile-host tests; chosen strategy documented in ADR. | Architecture/plugin owner |
| **DR-2026-004** | Which GUI technology is frozen? | **OPEN** | Do **not** freeze yet; select only after embedding + standalone + HiDPI + lifecycle/threading + packaging evaluation. | Prototype on target platforms; latency/lifecycle tests; packaging test; accessibility/scaling review; ADR acceptance. | GUI owner |
| **DR-2026-005** | Which OS/architectures/DAWs/versions are officially supported? | **OPEN** | Engineering baseline: Windows 11 x86_64; macOS supported Intel + Apple Silicon baseline; Ubuntu 24.04 LTS x86_64. MVP formats: CLAP + VST3. Pin exact supported DAW/version list before compatibility testing. | Product-approved support matrix with exact OS/build + DAW/version entries and test obligations. | Product owner |
| **DR-2026-006** | What is the measurable performance target? | **OPEN** | Keep the provisional CPU objective only as a hypothesis; freeze a benchmark protocol before turning it into a release requirement. | Reference machine(s), OS, toolchain, sample rate, block sizes, 8-band workload, automation pattern, warm-up, repetitions and acceptance threshold recorded. | Performance owner |

## Fast decision rule

1. **Accept** only if the recommendation and evidence are sufficient for the owner to sign off.
2. **Reject/modify** by recording the alternative in the corresponding DR and ADR before implementation.
3. **Leave OPEN** if evidence is insufficient.
4. Any unresolved **P0** contradiction keeps **M0 BLOCKED**.
5. No AI or developer may convert `OPEN` to `PASS` by inference.

## M0 closure condition

M0 may become **PASS** only when:

- DR-2026-001 and DR-2026-005 have explicit product/legal decisions;
- all remaining P0 architectural decisions required by the M0 contract are recorded in their ADRs;
- M0-G-001 through M0-G-009 are `PASS`;
- evidence is attached to each gate criterion;
- `EXECUTION_STATE.md` records the gate decision and next READY task.

## Evidence note

The current decision pack identifies EUIPO eSearch/TMview and national trademark databases as required evidence for the name decision. EUIPO explicitly warns that availability searches reduce risk but do not eliminate the possibility of challenge.  

For VST3, the current Steinberg documentation confirms that VST3 is MIT-licensed since version 3.8 and that the SDK includes a command-line validator/test host. This supports the adapter recommendation but does not by itself select the final RomuEq plugin architecture.
