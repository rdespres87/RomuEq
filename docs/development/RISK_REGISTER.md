# Risk Register

| ID | Risk | Priority | Trigger | Mitigation | Owner | Status |
|---|---|---|---|---|---|---|
| RISK-001 | Product name/trademark conflict | P0 | Search finds conflict | Formal clearance before freeze | Product owner | OPEN |
| RISK-002 | Parameter queue implementation not proven | P0 | Ordering/overflow/concurrency failure | Prototype + stress/model checking | Architecture owner | OPEN |
| RISK-003 | Plugin framework maintenance/licensing risk | P0 | Framework fails evidence or audit | Compare official SDK path and maintained framework options | Architecture owner | OPEN |
| RISK-004 | GUI framework lifecycle/real-time interaction risk | P1 | Host/editor lifecycle failure | Dedicated prototype and host tests | GUI owner | OPEN |
| RISK-005 | Host behavior differs from assumed contract | P0 | Host test failure | Freeze versioned matrix and adapter behavior | Plugin owner | OPEN |
| RISK-006 | Performance target not reproducible | P1 | Variance exceeds protocol | Freeze reference machine/protocol | Performance owner | OPEN |
| RISK-007 | State migration breakage | P0 | Old fixture fails | Versioned schema + migration fixtures | Engine owner | OPEN |
| RISK-008 | Sample-accurate automation degraded to block rate | P0 | Offset mismatch | Separate host event stream + offset tests | Engine/plugin owner | OPEN |
| RISK-009 | Numerical instability at extreme parameters | P0 | NaN/Inf or unstable poles | Clamp/validate + analytical/reference tests | DSP owner | OPEN |
| RISK-010 | Release artifact not reproducible | P0 | Hash/environment mismatch | Locked builds + clean-machine verification | Release owner | OPEN |
| RISK-011 | Supply-chain vulnerability | P1 | `cargo deny` or advisory failure | Audit/update/review dependency | Maintainer | OPEN |
| RISK-012 | Scope creep during implementation | P1 | Unplanned file/feature changes | Task protocol + DR mechanism | All | OPEN |

## Risk rules

Any new P0 risk blocks the affected gate until mitigated or explicitly accepted by the authorized owner. P1 risks require owner and remediation plan. P2 risks may be tracked without blocking unless promoted by evidence.
