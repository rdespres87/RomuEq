# Task Registry Schema v2

`docs/development/tasks/tasks.json` is a JSON array of exactly 68 task objects. Legacy execution fields remain authoritative for machine compatibility: `steps`, `tests`, `commands`, `acceptance`, `evidence`, `allowed`, `forbidden`, `rollback`.

For M1-M11, the following additional fields are mandatory: `purpose`, `preconditions`, `implementation_steps`, `validation_commands`, `acceptance_criteria`, `allowed_paths`, `forbidden_paths`, `evidence_artifacts`, `stop_the_line`, `rollback_failure_handling`, `done_definition`, `delegation_notes`. Their values must agree with the authoritative legacy fields where they overlap.

Status vocabulary: `BLOCKED`, `READY`, `IN_PROGRESS`, `IMPLEMENTED`, `TESTING`, `REVIEW`, `GATE_CHECK`, `DONE`.

Dependency references must resolve to task IDs and the dependency graph must be acyclic. A task may become `DONE` only after its acceptance criteria and required evidence are complete.
