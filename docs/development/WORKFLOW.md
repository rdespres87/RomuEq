# Development Workflow

See [`DEVELOPMENT_PLAN.md`](DEVELOPMENT_PLAN.md) for the milestone-based development plan and 99% readiness gates.

# Development Workflow

## Branching

- `main`: protected and releasable;
- short-lived feature branches;
- pull requests required.

## Before opening a PR

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-targets
cargo deny check
```

## Commit discipline

Prefer small commits grouped by invariant or architectural decision.

Avoid mixing DSP behavior, dependency upgrades, formatting-only changes and plugin adapter changes in one PR unless there is a strong reason.

## Architecture changes

Any P0 architectural change requires an ADR update and new evidence.


## Deterministic execution

For manual or AI execution, follow:

1. [`MASTER_PLAN.md`](MASTER_PLAN.md)
2. [`AI_DEVELOPMENT_PROTOCOL.md`](AI_DEVELOPMENT_PROTOCOL.md)
3. [`tasks/TASKS.md`](tasks/TASKS.md)
4. [`MILESTONE_GATE.md`](MILESTONE_GATE.md)

Run the hardening consistency check before beginning implementation:

```bash
python3 scripts/validate_m05.py
```


## Deterministic execution

For manual or AI execution, follow:

1. [`MASTER_PLAN.md`](MASTER_PLAN.md)
2. [`AI_DEVELOPMENT_PROTOCOL.md`](AI_DEVELOPMENT_PROTOCOL.md)
3. [`tasks/TASKS.md`](tasks/TASKS.md)
4. [`MILESTONE_GATE.md`](MILESTONE_GATE.md)

Run the hardening consistency check before beginning implementation:

```bash
python3 scripts/validate_m05.py
```
