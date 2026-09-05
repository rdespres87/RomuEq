# M0 Decision Status — 2026-09-03

The product owner explicitly approved the recommendations presented in the M0 one-page decision matrix.

| DR | Decision status | Recorded decision |
|---|---|---|
| DR-2026-001 | ACCEPTED (policy) | RomuEq remains the engineering/public working name; no public/commercial release under the name until formal clearance. Formal clearance is a release gate, not an engineering blocker. |
| DR-2026-002 | ACCEPTED | Use bounded preallocated SPSC control-command transport plus a separate sample-offset-preserving host automation stream. Exact queue implementation remains subject to M2 prototype evidence. |
| DR-2026-003 | ACCEPTED | Use isolated CLAP/VST3 adapters; do not freeze the core around an unverified third-party framework. |
| DR-2026-004 | ACCEPTED (deferred selection) | Do not freeze GUI framework before the required embedding/standalone/HiDPI/lifecycle/threading/packaging evaluation. |
| DR-2026-005 | ACCEPTED (baseline) | Freeze MVP platforms/formats as macOS ARM64/x64, Windows x64, Linux x64 with CLAP/VST3. Exact DAW/version test entries are deferred to the M7 compatibility freeze. |
| DR-2026-006 | ACCEPTED (provisional) | Freeze benchmark protocol and reference machine before converting the provisional CPU objective into a release requirement. |

## Gate consequence

M0 is no longer blocked by DR-2026-001/005 policy decisions. Formal name clearance remains a release gate; exact DAW/version entries are a pre-M7 test-matrix deliverable.
