# Decision Request Standard

A Decision Request (DR) is the only approved mechanism for unresolved architecture, product, legal, compatibility or scope questions.

## Naming

`DR-YYYY-NNN` where `YYYY` is the year and `NNN` is a sequential number.

## Required fields

```text
ID:
Title:
Status: OPEN | ACCEPTED | REJECTED | SUPERSEDED
Priority: P0 | P1 | P2
Owner:
Date opened:
Affected milestone(s):
Affected task(s):
Question:
Current evidence:
Options:
Recommendation:
Consequences:
Required experiments:
Decision deadline:
Decision:
Decision date:
```

## Rules

- P0 DRs block dependent P0 tasks.
- The AI may prepare evidence and a recommendation but may not mark a P0 DR `ACCEPTED` without the designated decision owner.
- Accepted decisions must update the relevant ADR and task dependencies.
- Superseding a decision requires a new DR and explicit migration impact.

## Initial P0 decision register

The following are known decision points inherited from the pre-production plan:

| DR | Topic | Status |
|---|---|---|
| DR-2026-001 | Final product/name clearance | ACCEPTED (policy) / CLEARANCE OPEN |
| DR-2026-002 | Exact parameter transport implementation | ACCEPTED |
| DR-2026-003 | Plugin framework selection/freeze | ACCEPTED |
| DR-2026-004 | GUI framework selection/freeze | ACCEPTED (deferred selection) |
| DR-2026-005 | Final P0 host/platform/version matrix | OPEN |
| DR-2026-006 | Final MVP performance budget and reference machine | ACCEPTED (provisional) |

These are intentionally explicit. No implementation task may silently close them.


## 2026-09-03 owner approval

The product owner approved the recommendations in `M0-DECISION-MATRIX.md`. This approval does not constitute legal trademark clearance and does not invent the missing exact DAW/version matrix. See `DECISION_STATUS-2026-09-03.md`.
