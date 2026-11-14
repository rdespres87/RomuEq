# Equaform Master Development Plan

**Status:** M0.5 hardened execution system — pre-production
**Product:** Equaform — provisional until formal name/trademark/domain clearance
**Engineering target:** evidence-backed readiness ≥99% before release
**Single source of execution truth:** this document + the controlled task registry under `docs/development/tasks/`

## 1. Purpose

This repository is designed to be executable by either a human engineer or an AI agent without silently inventing architecture, changing scope, or deciding unresolved P0 questions.

The master plan is a control system, not merely a roadmap. Every implementation change must map to a task, every task must have acceptance evidence, and every milestone must pass a gate before the next milestone becomes eligible.

## 2. Document precedence

When documents disagree, use this order:

1. Explicit product/legal decisions recorded as accepted ADRs.
2. This `MASTER_PLAN.md` and the milestone/task registry.
3. `AI_DEVELOPMENT_PROTOCOL.md` and `TASK_SPECIFICATION.md`.
4. Accepted ADRs under `docs/decisions/`.
5. `DEVELOPMENT_PLAN.md` for detailed technical rationale and background.
6. Component README files.
7. Source comments and implementation details.

If two documents at the same precedence level conflict, **STOP** and create a Decision Request. Do not guess.

## 3. Execution state machine

```text
BLOCKED -> READY -> IN_PROGRESS -> IMPLEMENTED -> TESTING -> REVIEW -> GATE_CHECK -> DONE
    ^                                                        |
    +--------------------------- FAIL <----------------------+
```

A task may enter `READY` only when all dependencies are `DONE`, required decisions are accepted, and its inputs are available.

A task is `DONE` only when its implementation, tests, evidence, documentation and acceptance criteria are complete.

## 4. Priority model

- **P0:** release/architecture/real-time correctness blocker. Any unresolved P0 blocks dependent work and can block the entire milestone.
- **P1:** important quality or compatibility requirement. May not block local development unless the gate says so.
- **P2:** improvement or polish. Never substitute for P0/P1 work.

## 5. Milestones

| ID | Milestone | Gate | Dependency |
|---|---|---|---|
| M0 | Naming, scope, product contract | M0 gate | — |
| M0.5 | Development system hardening | M0.5 gate | M0 |
| M1 | Repository, toolchain, CI | M1 gate | M0.5 |
| M2 | P0 architecture prototypes | M2 gate | M1 |
| M3 | DSP mathematical foundation | M3 gate | M2 |
| M4 | Real-time engine | M4 gate | M3 |
| M5 | Analysis pipeline | M5 gate | M4 |
| M6 | GUI and interaction model | M6 gate | M5 |
| M7 | Plugin adapters | M7 gate | M6 |
| M8 | Standalone + CLI | M8 gate | M7 |
| M9 | Compatibility and hostile-host testing | M9 gate | M8 |
| M10 | Performance, reliability, hardening | M10 gate | M9 |
| M11 | Release candidate | M11 gate | M10 |

## 6. 99% confidence rule

The release confidence score is evidence-weighted:

| Evidence domain | Weight |
|---|---:|
| Architecture | 20% |
| Tests | 20% |
| Real-time safety | 20% |
| Compatibility | 15% |
| Performance | 10% |
| Security / supply chain | 5% |
| Documentation / traceability | 10% |

A milestone or release fails automatically if any P0 requirement is unresolved, even if the weighted score is high. A release requires **score ≥99% and all mandatory gates PASS**.

The score is a review metric, not a substitute for evidence.

## 7. Standard execution loop

1. Read the current `EXECUTION_STATE.md`.
2. Select the first `READY` task in dependency order.
3. Read its task specification and linked ADRs.
4. Implement only within its allowed paths and scope.
5. Write tests before or with implementation according to the task's test strategy.
6. Run the task validation commands.
7. Produce the listed evidence artifacts.
8. Run the milestone gate when the task completes the milestone.
9. Update traceability, risks and state.
10. Commit only after the required checks pass.

## 8. Mandatory stop conditions

Stop immediately and create a Decision Request when:

- a P0 architecture decision is missing or contradicted;
- a task would require changing a public contract not listed in its scope;
- a dependency must be introduced without an approved dependency record;
- an unsafe block is required outside an already approved adapter boundary;
- a real-time invariant cannot be demonstrated;
- a host API behavior differs from the contract assumed by the task;
- tests expose an undocumented behavior change;
- the acceptance criteria cannot be met as written;
- a task would require changing another task's acceptance criteria;
- a release artifact is not reproducible.

## 9. No-scope-creep rule

Do not perform opportunistic refactors, dependency upgrades, naming changes, GUI redesigns, DSP substitutions, or API cleanup merely because they appear useful. Create a separate task or Decision Request.

## 10. Required artifacts per milestone

Every milestone produces:

- implementation report;
- test report;
- evidence index;
- risk review;
- decision status;
- gate result;
- updated traceability matrix;
- updated execution state.

## 11. Entry and exit control

A milestone cannot start until its predecessor gate is `PASS` and all P0 dependencies are resolved.

A milestone cannot close until `MILESTONE_GATE.md` is executed against the milestone's checklist and all mandatory evidence references are present.

## 12. Current state

M0.5 is the mandatory hardening package after the M0 product-contract gate. It does not silently resolve the open plugin-framework, GUI-framework, parameter-queue implementation, trademark, host-version, or final platform decisions. Those are explicit decision points.


## 13. Detailed local execution manual

The complete M1-M11 procedure, including per-task preconditions, exact implementation steps, tests, commands, acceptance criteria, evidence, forbidden paths, rollback and stop-the-line conditions, is maintained in `docs/development/TASK_EXECUTION_MANUAL.md`.


## 13. Detailed local execution manual

The complete M1-M11 procedure, including per-task preconditions, exact implementation steps, tests, commands, acceptance criteria, evidence, forbidden paths, rollback and stop-the-line conditions, is maintained in `docs/development/TASK_EXECUTION_MANUAL.md`.


## 13. Detailed local execution manual

The complete M1-M11 procedure, including per-task preconditions, exact implementation steps, tests, commands, acceptance criteria, evidence, forbidden paths, rollback and stop-the-line conditions, is maintained in `docs/development/TASK_EXECUTION_MANUAL.md`.
