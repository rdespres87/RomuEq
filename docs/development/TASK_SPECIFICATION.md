# Task Specification Standard

Every executable task uses this schema. A task that lacks any required field is not `READY`.

## Required fields

- `ID`
- `Title`
- `Status`
- `Priority`
- `Milestone`
- `Dependencies`
- `Decision dependencies`
- `Allowed paths`
- `Forbidden paths`
- `Inputs`
- `Implementation steps`
- `Tests`
- `Validation commands`
- `Acceptance criteria`
- `Evidence artifacts`
- `Gate impact`
- `Human decision required`
- `Rollback / failure handling`

## Status values

`BLOCKED`, `READY`, `IN_PROGRESS`, `IMPLEMENTED`, `TESTING`, `REVIEW`, `GATE_CHECK`, `DONE`, `FAILED`.

## Task template

```yaml
id: Mx-Txxx
title: Short imperative title
status: BLOCKED
priority: P0
milestone: Mx
dependencies: []
decision_dependencies: []
allowed_paths: []
forbidden_paths: []
inputs: []
implementation_steps: []
tests: []
validation_commands: []
acceptance_criteria: []
evidence_artifacts: []
gate_impact: P0|P1|P2
human_decision_required: NO
rollback_failure_handling: "..."
```

## Interpretation rules

- `allowed_paths` are the only repository areas the implementer may modify for the task.
- `forbidden_paths` override general repository write permissions.
- A dependency is complete only when its status is `DONE`.
- `decision_dependencies` must be accepted before a P0 task can become `READY`.
- Acceptance criteria are binary. “Mostly works” is not an accepted state.
- Evidence artifacts must be reproducible from the repository and the documented environment.
- `gate_impact: P0` means a failure can block the milestone.

## Task sizing

A task should normally be one coherent invariant or one bounded deliverable. If implementation would require unrelated architectural choices, split the task or create a Decision Request.

## Extended execution contract

For M1-M11, every task must additionally define:

- `purpose`: why the task exists and which risk it removes;
- `preconditions`: exact state required before starting;
- `preconditions`: must be checked against `EXECUTION_STATE.md`;
- `stop_the_line`: conditions that force `BLOCKED`/Decision Request;
- `done_definition`: binary completion requirements;
- `delegation_notes`: instructions preventing silent scope or architecture changes.

The detailed procedures live in `docs/development/TASK_EXECUTION_MANUAL.md`. The manual is an execution aid; accepted contracts and ADRs remain authoritative.

### Evidence rule

A test result without a reproducible command/environment is not sufficient evidence. A manual inspection claim must identify the inspected paths and invariant. A host compatibility claim must identify the exact host/version/OS/architecture. A benchmark claim must identify the frozen reference environment.

## Extended execution contract

For M1-M11, every task must additionally define:

- `purpose`: why the task exists and which risk it removes;
- `preconditions`: exact state required before starting;
- `preconditions`: must be checked against `EXECUTION_STATE.md`;
- `stop_the_line`: conditions that force `BLOCKED`/Decision Request;
- `done_definition`: binary completion requirements;
- `delegation_notes`: instructions preventing silent scope or architecture changes.

The detailed procedures live in `docs/development/TASK_EXECUTION_MANUAL.md`. The manual is an execution aid; accepted contracts and ADRs remain authoritative.

### Evidence rule

A test result without a reproducible command/environment is not sufficient evidence. A manual inspection claim must identify the inspected paths and invariant. A host compatibility claim must identify the exact host/version/OS/architecture. A benchmark claim must identify the frozen reference environment.

## Extended execution contract

For M1-M11, every task must additionally define:

- `purpose`: why the task exists and which risk it removes;
- `preconditions`: exact state required before starting;
- `preconditions`: must be checked against `EXECUTION_STATE.md`;
- `stop_the_line`: conditions that force `BLOCKED`/Decision Request;
- `done_definition`: binary completion requirements;
- `delegation_notes`: instructions preventing silent scope or architecture changes.

The detailed procedures live in `docs/development/TASK_EXECUTION_MANUAL.md`. The manual is an execution aid; accepted contracts and ADRs remain authoritative.

### Evidence rule

A test result without a reproducible command/environment is not sufficient evidence. A manual inspection claim must identify the inspected paths and invariant. A host compatibility claim must identify the exact host/version/OS/architecture. A benchmark claim must identify the frozen reference environment.
