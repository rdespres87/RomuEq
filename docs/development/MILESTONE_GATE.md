# Milestone Gate Standard

## Gate result values

- `PASS`: all mandatory criteria and evidence are satisfied.
- `FAIL`: at least one mandatory criterion is unsatisfied.
- `BLOCKED`: a P0 decision/evidence dependency is unavailable.
- `CONDITIONAL`: only allowed for non-release P2 items explicitly recorded with owner and deadline.

`CONDITIONAL` is never valid for M2, M3, M4, M7, M9, M10 or M11 P0 criteria.

## Gate procedure

1. Confirm predecessor gate is `PASS`.
2. Confirm all milestone tasks are `DONE` or explicitly exempted by a recorded decision.
3. Run the milestone validation commands.
4. Inspect evidence artifacts.
5. Reconcile traceability.
6. Review the risk register.
7. Review open Decision Requests.
8. Calculate the confidence score.
9. Record `PASS`, `FAIL` or `BLOCKED` in the milestone report.
10. Update `EXECUTION_STATE.md`.

## Mandatory gate report

Each gate report must contain:

- milestone;
- date and environment;
- task completion summary;
- commands and exact results;
- evidence artifact references;
- P0/P1/P2 open risks;
- Decision Request status;
- confidence score by evidence domain;
- final gate result;
- reviewer/sign-off identity.

## Automatic failure conditions

- any P0 task not `DONE`;
- any unresolved P0 Decision Request;
- any real-time safety regression;
- public state/parameter compatibility regression;
- failed mandatory CI check;
- unverified plugin lifecycle behavior at a required gate;
- missing required evidence;
- reproducibility failure for a release artifact.

## Confidence scoring

Use the weights in `MASTER_PLAN.md`. Each domain receives a 0–100 evidence score based on completed tests and independent review. Weighted score must be at least 99 for final release.

Do not round a score upward to meet the threshold.
