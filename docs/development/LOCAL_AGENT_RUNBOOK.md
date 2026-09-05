# Local Agent Runbook

## Startup

1. `git status --short` and record revision.
2. `cat rust-toolchain.toml`.
3. Read `EXECUTION_STATE.md`; never infer current state from chat history.
4. Validate the registry with `python3 scripts/validate_execution_plan.py`.
5. Read only the task selected by the registry plus its linked contracts/ADRs.

## Context-efficient reading

Do not reread the whole repository for every task. Read the global contracts once per milestone, then task-specific files and impacted code. Re-read a contract whenever a change could alter its invariant.

## Task selection algorithm

1. Filter tasks with status `READY`.
2. Keep only tasks whose `deps` are all `DONE`.
3. Order by milestone then task ID.
4. Select exactly one.
5. If zero tasks are selectable, distinguish `GATE_CHECK`, `BLOCKED`, or registry inconsistency; do not invent work.

## Failure recovery

- Compile/test failure: remain in testing/review, capture output, fix root cause, rerun.
- Contract conflict: BLOCKED + Decision Request.
- Environment/tool missing: BLOCKED unless the manual explicitly permits an alternate command.
- Scope discovery: do not expand task; create follow-up task or DR.
- Regression outside touched area: stop, preserve evidence, diagnose before proceeding.

## Completion checklist

A task is DONE only when implementation, tests, acceptance, evidence, traceability/risk/state updates and stop-the-line checks all pass.

## Milestone completion

After the last task is DONE, run the milestone gate. Never mark the next milestone READY until the gate is PASS and the registry/state files agree.

## Release discipline

Do not publish. Do not claim supported hosts. Do not claim trademark clearance. Do not sign artifacts unless the release procedure and required credentials/authority are available. M11 is the only release decision point.
