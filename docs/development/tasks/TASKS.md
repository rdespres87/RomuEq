# RomuEq Executable Task Registry

> **Detailed local execution:** see `docs/development/TASK_EXECUTION_MANUAL.md`. For M1-M11, `tasks.json` now carries purpose, preconditions, expanded implementation steps, tests, validation commands, acceptance criteria, evidence, stop-the-line rules, rollback and DONE definition. The registry remains the status/dependency authority.

| ID | Status | Priority | Title | Dependencies | Decision dependencies | Human decision | Gate |
|---|---|---|---|---|---|---|---|
| M0.5-T001 | DONE | P0 | Establish execution-state and task registry | M0-T005 | — | NO | M0.5 |
| M0.5-T002 | DONE | P0 | Install document precedence and stop-the-line rules | M0.5-T001 | — | NO | M0.5 |
| M0.5-T003 | DONE | P0 | Create Decision Request register | M0.5-T002 | — | NO | M0.5 |
| M0.5-T004 | DONE | P0 | Create milestone gate procedure | M0.5-T003 | — | NO | M0.5 |
| M0.5-T005 | DONE | P0 | Create test/evidence/traceability system | M0.5-T004 | — | NO | M0.5 |
| M0.5-T006 | DONE | P1 | Create compatibility/performance/release control documents | M0.5-T005 | — | NO | M0.5 |
| M0.5-T007 | DONE | P0 | Convert M0-M11 roadmap into executable task registry | M0.5-T006 | — | NO | M0.5 |
| M0.5-T008 | DONE | P0 | Run M0.5 consistency audit and close gate package | M0.5-T007 | — | NO | M0.5 |
| M0-T001 | DONE | P0 | Freeze product scope and MVP contract | — | DR-2026-001 | YES | M0 |
| M0-T002 | DONE | P0 | Freeze platform and format targets | M0-T001 | DR-2026-005 | YES | M0 |
| M0-T003 | DONE | P0 | Freeze parameter/state contract | M0-T002 | — | NO | M0 |
| M0-T004 | DONE | P0 | Freeze licensing and dependency policy | M0-T003 | — | NO | M0 |
| M0-T005 | DONE | P0 | Run M0 product-contract gate | M0-T004 | — | NO | M0 |
| M1-T001 | READY | P0 | Pin Rust toolchain and workspace policy | M0.5-T008 | — | NO | M1 |
| M1-T002 | BLOCKED | P0 | Establish CI validation ladder | M1-T001 | — | NO | M1 |
| M1-T003 | BLOCKED | P1 | Establish dependency/security checks | M1-T002 | — | NO | M1 |
| M1-T004 | BLOCKED | P0 | Establish cross-platform build matrix | M1-T003 | DR-2026-005 | YES | M1 |
| M1-T005 | BLOCKED | P0 | Run M1 repository gate | M1-T004 | — | NO | M1 |
| M2-T001 | BLOCKED | P0 | Prototype bounded parameter transport | M1-T005 | DR-2026-002 | YES | M2 |
| M2-T002 | BLOCKED | P0 | Prototype host automation event ingestion | M2-T001 | — | NO | M2 |
| M2-T003 | BLOCKED | P0 | Prototype engine/plugin boundary | M2-T002 | DR-2026-003 | YES | M2 |
| M2-T004 | BLOCKED | P0 | Prototype audio buffer and lifecycle contract | M2-T003 | — | NO | M2 |
| M2-T005 | BLOCKED | P0 | Run M2 architecture evidence gate | M2-T004 | — | NO | M2 |
| M3-T001 | BLOCKED | P0 | Implement parameter types and stable IDs | M2-T005 | — | NO | M3 |
| M3-T002 | BLOCKED | P0 | Implement biquad coefficient/reference math | M3-T001 | — | NO | M3 |
| M3-T003 | BLOCKED | P0 | Implement filter state/reset semantics | M3-T002 | — | NO | M3 |
| M3-T004 | DONE | P0 | Implement state/preset schema and migrations | M3-T003 | — | NO | M3 |
| M3-T005 | DONE | P0 | Run M3 DSP gate | M3-T004 | — | NO | M3 |
| M4-T001 | BLOCKED | P0 | Implement allocation-free engine lifecycle | M3-T005 | — | NO | M4 |
| M4-T002 | BLOCKED | P0 | Implement block processing and variable block sizes | M4-T001 | — | NO | M4 |
| M4-T003 | BLOCKED | P0 | Implement sample-accurate parameter application | M4-T002 | — | NO | M4 |
| M4-T004 | BLOCKED | P0 | Implement bypass/reprepare behavior | M4-T003 | — | NO | M4 |
| M4-T005 | BLOCKED | P0 | Run M4 real-time safety gate | M4-T004 | — | NO | M4 |
|| M5-T001 | DONE | P1 | Implement analysis data path outside audio thread | M4-T005 | — | NO | M5 |
| M5-T002 | BLOCKED | P1 | Implement spectrum/FFT pipeline | M5-T001 | — | NO | M5 |
| M5-T003 | BLOCKED | P1 | Implement meter computation and transport | M5-T002 | — | NO | M5 |
| M5-T004 | BLOCKED | P1 | Implement analysis configuration/state boundaries | M5-T003 | — | NO | M5 |
| M5-T005 | BLOCKED | P1 | Run M5 analysis gate | M5-T004 | — | NO | M5 |
| M6-T001 | BLOCKED | P0 | Freeze GUI framework and lifecycle contract | M5-T005 | DR-2026-004 | YES | M6 |
| M6-T002 | BLOCKED | P1 | Implement parameter editor and band controls | M6-T001 | — | NO | M6 |
| M6-T003 | BLOCKED | P1 | Implement graph/spectrum presentation | M6-T002 | — | NO | M6 |
| M6-T004 | BLOCKED | P1 | Implement preset/state UI interactions | M6-T003 | — | NO | M6 |
| M6-T005 | BLOCKED | P1 | Run M6 GUI gate | M6-T004 | — | NO | M6 |
| M7-T001 | BLOCKED | P0 | Implement CLAP adapter | M6-T005 | DR-2026-003 | YES | M7 |
| M7-T002 | BLOCKED | P0 | Implement VST3 adapter | M7-T001 | DR-2026-003 | YES | M7 |
| M7-T003 | BLOCKED | P0 | Implement plugin state/automation mapping | M7-T002 | — | NO | M7 |
| M7-T004 | BLOCKED | P0 | Validate plugin lifecycle and editor embedding | M7-T003 | — | NO | M7 |
| M7-T005 | BLOCKED | P0 | Run M7 plugin gate | M7-T004 | — | NO | M7 |
| M8-T001 | BLOCKED | P1 | Implement standalone host wrapper | M7-T005 | — | NO | M8 |
| M8-T002 | BLOCKED | P1 | Implement CLI offline renderer | M8-T001 | — | NO | M8 |
| M8-T003 | BLOCKED | P1 | Implement offline reference comparisons | M8-T002 | — | NO | M8 |
| M8-T004 | BLOCKED | P1 | Implement packaging scripts | M8-T003 | — | NO | M8 |
| M8-T005 | BLOCKED | P1 | Run M8 standalone/CLI gate | M8-T004 | — | NO | M8 |
| M9-T001 | BLOCKED | P0 | Freeze exact host compatibility matrix | M8-T005 | DR-2026-005 | YES | M9 |
| M9-T002 | BLOCKED | P0 | Execute scan/instantiate/lifecycle matrix | M9-T001 | — | NO | M9 |
| M9-T003 | BLOCKED | P0 | Execute automation/state/bypass matrix | M9-T002 | — | NO | M9 |
| M9-T004 | BLOCKED | P0 | Execute hostile block/rate/offline scenarios | M9-T003 | — | NO | M9 |
| M9-T005 | BLOCKED | P0 | Run M9 compatibility gate | M9-T004 | — | NO | M9 |
| M10-T001 | BLOCKED | P0 | Freeze performance reference environment | M9-T005 | DR-2026-006 | YES | M10 |
| M10-T002 | BLOCKED | P0 | Run CPU/memory/allocation benchmarks | M10-T001 | — | NO | M10 |
| M10-T003 | BLOCKED | P0 | Run fuzz/property/stress suite | M10-T002 | — | NO | M10 |
| M10-T004 | BLOCKED | P0 | Run security/supply-chain/reproducibility checks | M10-T003 | — | NO | M10 |
| M10-T005 | BLOCKED | P0 | Run M10 hardening gate | M10-T004 | — | NO | M10 |
| M11-T001 | BLOCKED | P0 | Assemble release artifacts | M10-T005 | — | NO | M11 |
| M11-T002 | BLOCKED | P0 | Run clean-machine smoke tests | M11-T001 | — | NO | M11 |
| M11-T003 | BLOCKED | P0 | Run signing/notarization/distribution checks | M11-T002 | — | NO | M11 |
| M11-T004 | BLOCKED | P0 | Publish release evidence and traceability snapshot | M11-T003 | — | NO | M11 |
| M11-T005 | BLOCKED | P0 | Run final 99% release gate | M11-T004 | — | NO | M11 |

---

## Full task specifications
### M0.5-T001 — Establish execution-state and task registry

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0-T005
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Create and maintain task state files using the standard schema.
2. Verify dependency ordering and status vocabulary.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] Every task has a unique ID, status, priority and dependency list.
- [ ] A deterministic first READY task can be selected.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T002 — Install document precedence and stop-the-line rules

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T001
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Publish precedence rules.
2. Add explicit P0 stop conditions to the AI protocol.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] Conflicting instructions have a defined resolution path.
- [ ] P0 ambiguity causes BLOCKED state.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T003 — Create Decision Request register

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T002
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Register all known unresolved P0 decisions.
2. Link decision dependencies to affected tasks.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] No known P0 decision remains implicit.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T004 — Create milestone gate procedure

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T003
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Define PASS/FAIL/BLOCKED and automatic failure conditions.
2. Define evidence and confidence scoring.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] Every milestone has a reproducible gate procedure.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T005 — Create test/evidence/traceability system

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T004
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Publish test pyramid and evidence requirements.
2. Create initial requirement-to-test mappings.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] P0 requirements have verification paths and evidence expectations.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T006 — Create compatibility/performance/release control documents

- **Status:** `DONE`
- **Priority:** `P1`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T005
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Publish version-free planning matrices.
2. Define measurement and release evidence protocols.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] No release claim relies on undefined host versions or undocumented performance measurements.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T007 — Convert M0-M11 roadmap into executable task registry

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T006
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Create atomic tasks with dependencies, paths, tests and acceptance criteria.
2. Link task families to milestones and gates.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] All milestones have executable tasks.
- [ ] No task requires silent architecture decisions.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0.5-T008 — Run M0.5 consistency audit and close gate package

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0.5`
- **Dependencies:** M0.5-T007
- **Decision dependencies:** None
- **Allowed paths:** docs/development/, docs/development/tasks/
- **Forbidden paths:** src/, crates/
- **Inputs:** Existing DEVELOPMENT_PLAN.md and ADR set
- **Human decision required:** `NO`
- **Gate impact:** `M0.5`

**Implementation steps**
1. Check unique IDs, dependency references, required fields and document links.
2. Record gate result.

**Tests**
- Markdown/schema consistency checks

**Validation commands**
```text
python3 scripts/validate_m05.py
```

**Acceptance criteria**
- [ ] Audit passes with zero orphan dependencies and zero missing required fields.

**Evidence artifacts**
- docs/development/EXECUTION_STATE.md

**Rollback / failure handling**
Revert only files created by this task; preserve previous plan.

### M0-T001 — Freeze product scope and MVP contract

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0`
- **Dependencies:** None
- **Decision dependencies:** DR-2026-001 — accepted as provisional brand-use policy; formal clearance remains a release/M0 gate blocker.
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M0`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M0-T002 — Freeze platform and format targets

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M0`
- **Dependencies:** M0-T001
- **Decision dependencies:** DR-2026-005
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M0`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M0-T003 — Freeze parameter/state contract

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M0`
- **Dependencies:** M0-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M0`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M0-T003 completion evidence

- `docs/product/PARAMETER_STATE_CONTRACT.md` created and marked FROZEN FOR MVP.
- `docs/decisions/ADR-005-automation.md` accepted.
- `docs/decisions/ADR-006-state-format.md` accepted.
- Stable ID allocation, logical ranges, enum values, automation ordering, state schema version and migration rules are explicit.

### M0-T004 — Freeze licensing and dependency policy

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M0`
- **Dependencies:** M0-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M0`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M0-T004 completion evidence

- `docs/development/LICENSING_DEPENDENCY_POLICY.md` frozen for MVP.
- Existing `deny.toml` baseline reviewed against the policy.
- External license facts checked against current official Steinberg VST3 and CLAP sources; repository policy does not claim Cargo execution evidence.

### M0-T005 — Run M0 product-contract gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M0`
- **Dependencies:** M0-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M0`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M1-T001 — Pin Rust toolchain and workspace policy

- **Status:** `READY`
- **Priority:** `P0`
- **Milestone:** `M1`
- **Dependencies:** M0.5-T008
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M1`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M1-T002 — Establish CI validation ladder

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M1`
- **Dependencies:** M1-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M1`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M1-T003 — Establish dependency/security checks

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M1`
- **Dependencies:** M1-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M1`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M1-T004 — Establish cross-platform build matrix

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M1`
- **Dependencies:** M1-T003
- **Decision dependencies:** DR-2026-005
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M1`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M1-T005 — Run M1 repository gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M1`
- **Dependencies:** M1-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M1`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M2-T001 — Prototype bounded parameter transport

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M2`
- **Dependencies:** M1-T005
- **Decision dependencies:** DR-2026-002
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M2`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M2-T002 — Prototype host automation event ingestion

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M2`
- **Dependencies:** M2-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M2`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M2-T003 — Prototype engine/plugin boundary

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M2`
- **Dependencies:** M2-T002
- **Decision dependencies:** DR-2026-003
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M2`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M2-T004 — Prototype audio buffer and lifecycle contract

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M2`
- **Dependencies:** M2-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M2`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M2-T005 — Run M2 architecture evidence gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M2`
- **Dependencies:** M2-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M2`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M3-T001 — Implement parameter types and stable IDs

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M3`
- **Dependencies:** M2-T005
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M3`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M3-T002 — Implement biquad coefficient/reference math

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M3`
- **Dependencies:** M3-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M3`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M3-T003 — Implement filter state/reset semantics

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M3`
- **Dependencies:** M3-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M3`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M3-T004 — Implement state/preset schema and migrations

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M3`
- **Dependencies:** M3-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M3`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M3-T005 — Run M3 DSP gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M3`
- **Dependencies:** M3-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M3`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M4-T001 — Implement allocation-free engine lifecycle

- **Status:** `DONE`
- **Priority:** `P0`
- **Milestone:** `M4`
- **Dependencies:** M3-T005
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M4`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M4-T002 — Implement block processing and variable block sizes

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M4`
- **Dependencies:** M4-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M4`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M4-T003 — Implement sample-accurate parameter application

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M4`
- **Dependencies:** M4-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M4`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M4-T004 — Implement bypass/reprepare behavior

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M4`
- **Dependencies:** M4-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M4`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M4-T005 — Run M4 real-time safety gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M4`
- **Dependencies:** M4-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M4`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M5-T001 — Implement analysis data path outside audio thread

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M5`
- **Dependencies:** M4-T005
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M5`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M5-T002 — Implement spectrum/FFT pipeline

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M5`
- **Dependencies:** M5-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M5`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M5-T003 — Implement meter computation and transport

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M5`
- **Dependencies:** M5-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M5`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M5-T004 — Implement analysis configuration/state boundaries

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M5`
- **Dependencies:** M5-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M5`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M5-T005 — Run M5 analysis gate

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M5`
- **Dependencies:** M5-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M5`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M6-T001 — Freeze GUI framework and lifecycle contract

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M6`
- **Dependencies:** M5-T005
- **Decision dependencies:** DR-2026-004
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M6`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M6-T002 — Implement parameter editor and band controls

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M6`
- **Dependencies:** M6-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M6`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M6-T003 — Implement graph/spectrum presentation

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M6`
- **Dependencies:** M6-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M6`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M6-T004 — Implement preset/state UI interactions

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M6`
- **Dependencies:** M6-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M6`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M6-T005 — Run M6 GUI gate

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M6`
- **Dependencies:** M6-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M6`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M7-T001 — Implement CLAP adapter

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M7`
- **Dependencies:** M6-T005
- **Decision dependencies:** DR-2026-003
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M7`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M7-T002 — Implement VST3 adapter

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M7`
- **Dependencies:** M7-T001
- **Decision dependencies:** DR-2026-003
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M7`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M7-T003 — Implement plugin state/automation mapping

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M7`
- **Dependencies:** M7-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M7`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M7-T004 — Validate plugin lifecycle and editor embedding

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M7`
- **Dependencies:** M7-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M7`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M7-T005 — Run M7 plugin gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M7`
- **Dependencies:** M7-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M7`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M8-T001 — Implement standalone host wrapper

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M8`
- **Dependencies:** M7-T005
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M8`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M8-T002 — Implement CLI offline renderer

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M8`
- **Dependencies:** M8-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M8`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M8-T003 — Implement offline reference comparisons

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M8`
- **Dependencies:** M8-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M8`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M8-T004 — Implement packaging scripts

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M8`
- **Dependencies:** M8-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M8`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M8-T005 — Run M8 standalone/CLI gate

- **Status:** `BLOCKED`
- **Priority:** `P1`
- **Milestone:** `M8`
- **Dependencies:** M8-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M8`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M9-T001 — Freeze exact host compatibility matrix

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M9`
- **Dependencies:** M8-T005
- **Decision dependencies:** DR-2026-005
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M9`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M9-T002 — Execute scan/instantiate/lifecycle matrix

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M9`
- **Dependencies:** M9-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M9`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M9-T003 — Execute automation/state/bypass matrix

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M9`
- **Dependencies:** M9-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M9`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M9-T004 — Execute hostile block/rate/offline scenarios

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M9`
- **Dependencies:** M9-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M9`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M9-T005 — Run M9 compatibility gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M9`
- **Dependencies:** M9-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M9`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M10-T001 — Freeze performance reference environment

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M10`
- **Dependencies:** M9-T005
- **Decision dependencies:** DR-2026-006
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `YES`
- **Gate impact:** `M10`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M10-T002 — Run CPU/memory/allocation benchmarks

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M10`
- **Dependencies:** M10-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M10`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M10-T003 — Run fuzz/property/stress suite

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M10`
- **Dependencies:** M10-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M10`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M10-T004 — Run security/supply-chain/reproducibility checks

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M10`
- **Dependencies:** M10-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M10`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M10-T005 — Run M10 hardening gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M10`
- **Dependencies:** M10-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M10`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M11-T001 — Assemble release artifacts

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M11`
- **Dependencies:** M10-T005
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M11`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M11-T002 — Run clean-machine smoke tests

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M11`
- **Dependencies:** M11-T001
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M11`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M11-T003 — Run signing/notarization/distribution checks

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M11`
- **Dependencies:** M11-T002
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M11`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M11-T004 — Publish release evidence and traceability snapshot

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M11`
- **Dependencies:** M11-T003
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M11`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.

### M11-T005 — Run final 99% release gate

- **Status:** `BLOCKED`
- **Priority:** `P0`
- **Milestone:** `M11`
- **Dependencies:** M11-T004
- **Decision dependencies:** None
- **Allowed paths:** crates/, src/, docs/, .github/
- **Forbidden paths:** .git/, target/
- **Inputs:** Master plan, prior milestone outputs, linked ADRs
- **Human decision required:** `NO`
- **Gate impact:** `M11`

**Implementation steps**
1. Implement only the named deliverable.
2. Add/update tests before acceptance.
3. Update traceability and risk entries when required.

**Tests**
- Task-specific unit/integration tests
- Relevant regression fixtures

**Validation commands**
```text
cargo fmt --all -- --check
```
```text
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
```text
cargo test --workspace --all-features
```

**Acceptance criteria**
- [ ] All stated acceptance checks pass.
- [ ] No out-of-scope architectural change is introduced.

**Evidence artifacts**
- Task implementation report
- Test report
- Evidence index

**Rollback / failure handling**
If acceptance fails, return to TESTING or create a DR; do not widen scope.
