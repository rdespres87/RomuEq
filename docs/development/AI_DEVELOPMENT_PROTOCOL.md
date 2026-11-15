# AI Development Protocol

## Mission

Execute the Equaform repository deterministically from the task registry. The AI is an implementer and verifier, not an architecture owner.

## Absolute rules

1. Never invent a missing requirement.
2. Never resolve a P0 Decision Request by intuition.
3. Never weaken a test to make a task pass.
4. Never delete or skip a failing test without a documented decision.
5. Never change public IDs, state schema, automation semantics, audio-buffer semantics, latency semantics or plugin identifiers without an ADR and migration/evidence plan.
6. Never add a dependency without completing the dependency review fields in the task and passing `cargo deny check`.
7. Never place allocation, locks, I/O, logging, GUI work, syscalls or host calls in the audio hot path unless the relevant API contract explicitly permits them and the task proves safety.
8. Never use `unsafe` in a forbidden crate. If an adapter needs `unsafe`, stop unless the boundary is already approved.
9. Never expand a task's allowed paths without a Decision Request.
10. Never mark `DONE` without evidence.

## Deterministic task selection

Choose the lowest milestone number with an open task. Within that milestone choose the lowest task ID whose dependencies are `DONE` and whose blockers are cleared.

If multiple tasks are independent, prefer the task with P0 priority, then the task with the fewest downstream dependencies.

## Required task protocol

### Before implementation

- Read the task completely.
- Read every dependency task's result.
- Read linked ADRs.
- Verify allowed and forbidden paths.
- Confirm all prerequisite decisions are accepted.
- If anything is ambiguous, stop.

### During implementation

- Strictly adhere to the Spec-Driven TDD Protocol (`docs/development/TDD_METHODOLOGY.md`): Write failing tests first (Red), implement minimal passing code (Green), then optimize without allocations (Refactor).
- Keep changes local to the task.
- Preserve existing public contracts.
- Prefer the smallest safe implementation.
- Enforce core test coverage thresholds (>= 90% on DSP/Engine/Analysis/Common crates).
- Record any discovered issue in `RISK_REGISTER.md` rather than silently changing scope.

### After implementation

Run every command listed by the task, then the mandatory repository validation ladder applicable to the milestone.

Produce evidence with exact command, environment, result and artifact path. A statement such as “tests pass” without reproducible evidence is insufficient.

## Failure protocol

If a check fails:

1. Reproduce once without changing code.
2. Classify the failure: implementation, environment, flaky test, dependency, specification conflict, or external host behavior.
3. If implementation-related, fix only within task scope.
4. If specification/architecture-related, create `DR-xxx` and set the task `BLOCKED`.
5. If environmental, record the environment limitation and do not claim the gate passed.

## Architecture-change protocol

An architecture change requires:

- Decision Request;
- ADR update/new ADR;
- impact analysis;
- affected task list;
- updated traceability;
- new or updated tests;
- gate re-evaluation.

## Human decision points

Humans are required only when a Decision Request is marked `HUMAN_DECISION_REQUIRED: YES`, or when legal/product ownership is involved. The AI must not infer consent.

## Commit protocol

A commit message must identify the task, for example:

`M3-T014: add peaking biquad coefficient reference tests`

Do not combine unrelated task IDs in one commit unless the task registry explicitly permits a grouped change.
