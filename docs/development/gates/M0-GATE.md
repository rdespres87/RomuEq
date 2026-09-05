# M0 Gate — Naming, Scope and Product Contract

**Gate ID:** M0-GATE  
**Status:** `PASS`  
**Date:** 2026-09-03

## Purpose

M0 freezes the product contract before engineering implementation begins. No downstream milestone may start until this gate is `PASS`.

## PASS rule

M0 is `PASS` only when every mandatory criterion below is `PASS` and every P0 Decision Request required by the criteria is `ACCEPTED`.

Any `FAIL` or unresolved P0 item produces `BLOCKED`.

## Register

| ID | Criterion | Required evidence | Status |
|---|---|---|---|
| M0-G-001 | Product display name and working-brand policy frozen | Product contract + working-brand policy | PASS |
| M0-G-002 | Legal/trademark clearance decision recorded | Accepted risk/decision; formal clearance remains release gate | PASS |
| M0-G-003 | MVP scope frozen | Product contract | PASS |
| M0-G-004 | MVP processing contract frozen | Product contract | PASS |
| M0-G-005 | P0 platform/format targets frozen | DR-2026-005 accepted | PASS |
| M0-G-006 | Parameter/state contract frozen | Versioned parameter/state specification | PASS |
| M0-G-007 | Licensing/dependency policy frozen | Dependency/license policy + DRs | PASS |
| M0-G-008 | No unresolved P0 contradiction | Decision register + traceability | PASS |
| M0-G-009 | M0 evidence artifacts complete | Implementation/test/evidence reports | PASS |
| M0-G-010 | Repository execution state updated | EXECUTION_STATE.md | PASS |

## Deferred release / later-gate conditions

- Formal trademark/name clearance remains a release gate; no public/commercial release under the working name before clearance.
- Exact DAW/version compatibility entries are a M7 deliverable; M0 freezes only the platform/format baseline.

## Gate execution procedure

1. Verify all M0 tasks are `DONE`.
2. Verify all required P0 DRs are `ACCEPTED`.
3. Verify the product contract is versioned and internally consistent.
4. Verify traceability from product requirements to tasks and evidence.
5. Run the mandatory validation commands available in the environment.
6. Record evidence references for every criterion.
7. Set this gate to `PASS` only if all mandatory criteria pass.
8. Update `EXECUTION_STATE.md` and the M0 gate register.

## Stop-the-line

Do not proceed to M0.5, M1 or implementation work if this gate is `BLOCKED`.

If evidence reveals a new P0 decision, create a new `DR-YYYY-NNN` before proceeding.
