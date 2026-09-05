# RomuEq — Local Execution Manual (M1 → M11)

**Purpose:** make the remaining implementation executable by a local human or AI agent without relying on undocumented conversation context.

> This manual expands the controlled task registry. The registry, MASTER_PLAN, accepted ADRs and product contracts remain authoritative. This document never authorizes silent changes to unresolved P0 decisions.

## 0. Operating contract

### 0.1 Before every task
1. Read `docs/development/EXECUTION_STATE.md`.
2. Select the first `READY` task in dependency order.
3. Read this manual section, the corresponding task entry in `tasks.json`, and all linked ADR/product contracts.
4. Verify the working tree and toolchain.
5. Change only allowed paths.
6. Implement the smallest coherent change that satisfies the task.
7. Run the task tests, then the standard regression ladder.
8. Generate the evidence files named by the task.
9. Update traceability, risks and execution state.
10. Only then change the task to `DONE`.

### 0.2 Mandatory stop conditions
- Missing/contradictory P0 decision.
- Need to change an accepted product/parameter/state contract without a new decision.
- New dependency outside `LICENSING_DEPENDENCY_POLICY.md`.
- New unsafe code outside an approved adapter boundary.
- Allocation/blocking/I/O/syscall/panic in the audio hot path.
- Host behavior contradicts an assumption and no contract exists.
- Acceptance criterion cannot be objectively demonstrated.
- Test failure is flaky, unexplained or non-reproducible.
- Evidence cannot be reproduced from the documented environment.

### 0.3 Evidence naming
For each task, create:
- `docs/development/evidence/<TASK-ID>-implementation.md`
- `docs/development/evidence/<TASK-ID>-tests.md`
- `docs/development/evidence/<TASK-ID>-evidence.md`
Additional benchmark/decision/host evidence is listed in the task section.

### 0.4 Standard evidence record
```text
Task: <ID>
Commit/source revision: <value>
Environment: <OS/arch/toolchain>
Commands: <exact commands>
Results: PASS/FAIL
Artifacts: <paths>
Acceptance: criterion-by-criterion PASS/FAIL
Risks changed: <IDs>
Decisions changed: <DR/ADR IDs>
Known limitations: <none or explicit>
```

## 1. Global quality gates

| Area | Minimum evidence | Blocking rule |
|---|---|---|
| Build | workspace compiles on declared targets | P0 failure blocks |
| Format | rustfmt clean | P0 failure blocks |
| Clippy | `-D warnings` clean | P0 failure blocks |
| Tests | workspace regression green | P0 failure blocks |
| RT safety | zero process allocations + no blocking/I/O/panic | P0 failure blocks |
| DSP | reference/property/stability evidence | P0 failure blocks |
| Compatibility | exact host/version matrix | P0 failure blocks |
| Security | dependency/license/advisory/source checks | P0 failure blocks |
| Release | reproducible packages + clean-machine smoke | P0 failure blocks |

## 2. Detailed task execution specifications

### M1 — Repository, toolchain, CI

#### M1-T001 — Pin Rust toolchain and workspace policy

**Priority:** P0  
**Dependencies:** M0.5-T008  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Make the build environment explicit and reproducible before implementation.

**Preconditions**
- M0.5 gate PASS; repository checkout is clean enough to inspect.

**Implementation procedure**
1. Inspect rust-toolchain.toml, root Cargo.toml and every workspace manifest.
2. Pin an exact Rust toolchain version; do not silently change edition/resolver/package policy.
3. Ensure the workspace contains exactly the planned crates and no accidental members.
4. Ensure Cargo.lock policy is explicit and the lockfile is generated when appropriate for the workspace.
5. Document required rustfmt/clippy/toolchain components and local setup.
6. Record any unavailable local command as missing evidence rather than fabricating a result.

**Tests**
- Workspace membership check; manifest parse; format/clippy/test smoke when toolchain is available.

**Validation commands**
```bash
rustc --version
cargo --version
cargo metadata --no-deps --format-version 1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Exact toolchain is pinned.
- [ ] Workspace membership is deterministic.
- [ ] No placeholder repository identity is invented.
- [ ] The documented commands are runnable in the supported environment or explicitly marked pending external execution.
- [ ] No architecture is changed merely to satisfy tooling.

**Allowed paths**
- `Cargo.toml`
- `rust-toolchain.toml`
- `Cargo.lock`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M1-T001-implementation.md`
- `docs/development/evidence/M1-T001-tests.md`
- `docs/development/evidence/M1-T001-evidence.md`

**STOP-THE-LINE if**
- A required Rust version cannot build the declared edition/dependencies.
- Changing resolver/edition requires an unrecorded architecture/toolchain decision.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M1-T002 — Establish CI validation ladder

**Priority:** P0  
**Dependencies:** M1-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Create a CI ladder that fails early and makes every quality gate reproducible.

**Preconditions**
- M1-T001 DONE.

**Implementation procedure**
1. Define formatting, compile, clippy, unit/integration tests, dependency checks and platform jobs in increasing cost order.
2. Pin action versions/major versions according to repository policy.
3. Make failures visible and non-optional for P0 checks.
4. Separate fast PR checks from scheduled/nightly checks without weakening PR gates.
5. Store machine-readable reports where practical.

**Tests**
- CI syntax validation; local equivalent commands; intentional failure test on a disposable branch if feasible.

**Validation commands**
```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] PR CI executes required P0 checks.
- [ ] A failing P0 job blocks merge.
- [ ] Nightly jobs do not masquerade as release gates.
- [ ] Evidence paths are documented.

**Allowed paths**
- `.github/workflows/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M1-T002-implementation.md`
- `docs/development/evidence/M1-T002-tests.md`
- `docs/development/evidence/ci-run-M1-T002.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M1-T003 — Establish dependency/security checks

**Priority:** P1  
**Dependencies:** M1-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Enforce dependency, advisory, license and source policies before third-party code accumulates.

**Preconditions**
- M1-T002 DONE; LICENSING_DEPENDENCY_POLICY is authoritative.

**Implementation procedure**
1. Install/configure cargo-deny or equivalent only if allowed by policy.
2. Validate licenses, advisories, bans, duplicate versions and source origins.
3. Add dependency review for changes.
4. Record every exception through Decision Request rather than configuration drift.

**Tests**
- Dependency audit; license audit; source policy audit.

**Validation commands**
```bash
cargo deny check
cargo tree --workspace
```

**Acceptance — all must be PASS**
- [ ] No forbidden license/source/dependency class passes.
- [ ] All exceptions are explicitly recorded.
- [ ] Audit output is retained as evidence.

**Allowed paths**
- `deny.toml`
- `.github/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M1-T003-implementation.md`
- `docs/development/evidence/M1-T003-tests.md`
- `docs/development/evidence/M1-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M1-T004 — Establish cross-platform build matrix

**Priority:** P0  
**Dependencies:** M1-T003  
**Decision dependencies:** DR-2026-005  
**Human decision:** YES  

**Purpose**  Make the frozen platform baseline executable in CI/build scripts.

**Preconditions**
- M1-T003 DONE; accepted platform baseline from M0.

**Implementation procedure**
1. Define jobs for macOS ARM64/x64, Windows x64 and Linux x64.
2. Use only targets that are actually supported by the chosen toolchain/dependencies.
3. Build all core crates on each target; adapters may remain compile-only until their milestone.
4. Document runner/host limitations and do not claim host compatibility from cross-compilation alone.

**Tests**
- Per-target cargo check/test where available; artifact metadata inspection.

**Validation commands**
```bash
cargo build --workspace --all-targets
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All baseline targets compile.
- [ ] Unsupported test execution is explicitly marked as such.
- [ ] No unverified DAW compatibility is claimed.

**Allowed paths**
- `.github/workflows/`
- `docs/development/`
- `scripts/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M1-T004-implementation.md`
- `docs/development/evidence/M1-T004-tests.md`
- `docs/development/evidence/M1-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M1-T005 — Run M1 repository gate

**Priority:** P0  
**Dependencies:** M1-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close M1 with a reproducible repository/toolchain evidence package.

**Preconditions**
- M1-T001..T004 DONE.

**Implementation procedure**
1. Run the M1 checklist from clean checkout.
2. Collect versions, metadata, CI results, dependency audit and target results.
3. Update execution state, traceability and risk register.
4. Mark only demonstrated criteria PASS.

**Tests**
- Full M1 validation ladder.

**Validation commands**
```bash
python3 scripts/validate_m05.py
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All M1 P0 criteria PASS.
- [ ] Evidence references resolve.
- [ ] Next READY task is M2-T001 only after the gate.

**Allowed paths**
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M1-T005-implementation.md`
- `docs/development/evidence/M1-T005-tests.md`
- `docs/development/evidence/M1-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M2 — P0 architecture prototypes

#### M2-T001 — Prototype bounded parameter transport

**Priority:** P0  
**Dependencies:** M1-T005  
**Decision dependencies:** DR-2026-002  
**Human decision:** YES  

**Purpose**  Empirically select and validate the bounded control-thread parameter transport without changing the public contract.

**Preconditions**
- M1 gate PASS; ADR-001 remains Proposed.

**Implementation procedure**
1. Implement at least two candidate bounded SPSC designs if needed for comparison.
2. Measure ordering, capacity, overwrite/drop behavior, producer/consumer contention and latency.
3. Define deterministic overflow semantics.
4. Prove no allocation/blocking on the audio consumer side.
5. Prepare DR-2026-002 recommendation; do not freeze implementation before evidence.

**Tests**
- Concurrency stress; overflow; ordering; model checking where practical; allocation checks.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Exactly one implementation is recommended with evidence.
- [ ] Overflow is deterministic.
- [ ] No audio-thread allocation/blocking.
- [ ] ADR-001 can be updated only after the required decision is accepted.

**Allowed paths**
- `crates/romueq-engine/`
- `docs/development/`
- `docs/decisions/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M2-T001-implementation.md`
- `docs/development/evidence/M2-T001-tests.md`
- `docs/development/evidence/M2-T001-benchmark.md`
- `docs/development/evidence/M2-T001-decision-input.md`

**STOP-THE-LINE if**
- Candidate designs are indistinguishable without a human/product decision.
- Queue semantics would contradict stable parameter behavior.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M2-T002 — Prototype host automation event ingestion

**Priority:** P0  
**Dependencies:** M2-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Define the exact internal representation and ingestion path for host automation events.

**Preconditions**
- M2-T001 DONE.

**Implementation procedure**
1. Represent ParamId, logical value, sample_offset and deterministic sequence.
2. Validate offsets against block length.
3. Define behavior for duplicate offsets, out-of-order input and malformed values.
4. Keep host events separate from GUI/control commands.
5. Prove stable sorting/processing order.

**Tests**
- Random event sequences; identical offsets; malformed offsets; large event counts within bounded host input.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
```

**Acceptance — all must be PASS**
- [ ] All valid events preserve offsets.
- [ ] Ordering is deterministic by (sample_offset, sequence).
- [ ] Malformed input cannot corrupt state.
- [ ] No hidden block-rate downgrade.

**Allowed paths**
- `crates/romueq-engine/`
- `crates/romueq-common/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M2-T002-implementation.md`
- `docs/development/evidence/M2-T002-tests.md`
- `docs/development/evidence/M2-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M2-T003 — Prototype engine/plugin boundary

**Priority:** P0  
**Dependencies:** M2-T002  
**Decision dependencies:** DR-2026-003  
**Human decision:** YES  

**Purpose**  Prove the engine/plugin boundary keeps host APIs out of the core.

**Preconditions**
- M2-T002 DONE; DR-2026-003 still controls framework choice.

**Implementation procedure**
1. Define adapter-facing lifecycle interface.
2. Compile core without plugin dependencies.
3. Prototype CLAP and VST3 interaction points sufficient to validate the boundary.
4. Record framework candidates and evidence.
5. Do not freeze a framework without the decision gate.

**Tests**
- Core-only compile; adapter prototype compile; state/automation/lifecycle smoke.

**Validation commands**
```bash
cargo check -p romueq-engine
cargo test -p romueq-engine
```

**Acceptance — all must be PASS**
- [ ] Core has no host SDK dependency.
- [ ] Adapter owns host-specific types.
- [ ] Lifecycle ownership is explicit.
- [ ] DR-2026-003 contains enough evidence for a decision.

**Allowed paths**
- `crates/romueq-engine/`
- `crates/romueq-plugin/`
- `docs/development/`
- `docs/decisions/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M2-T003-implementation.md`
- `docs/development/evidence/M2-T003-tests.md`
- `docs/development/evidence/M2-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M2-T004 — Prototype audio buffer and lifecycle contract

**Priority:** P0  
**Dependencies:** M2-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Freeze the audio buffer and lifecycle contract experimentally before production DSP/engine code.

**Preconditions**
- M2-T003 DONE; ADR-004 remains Proposed.

**Implementation procedure**
1. Define planar f32 mono/stereo buffer representation.
2. Define prepare(sample_rate, max_block_size, channels) semantics.
3. Define process preconditions and invalid-host behavior.
4. Define reset/reprepare semantics.
5. Test variable block sizes including 1 sample and large blocks.

**Tests**
- Buffer shape/property tests; lifecycle sequence tests; invalid input tests.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Buffer contract is explicit.
- [ ] Invalid host behavior is safe and deterministic.
- [ ] prepare/reprepare/reset semantics are testable.

**Allowed paths**
- `crates/romueq-engine/`
- `crates/romueq-common/`
- `docs/decisions/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M2-T004-implementation.md`
- `docs/development/evidence/M2-T004-tests.md`
- `docs/development/evidence/M2-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M2-T005 — Run M2 architecture evidence gate

**Priority:** P0  
**Dependencies:** M2-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close architecture prototypes with evidence and accepted P0 decisions.

**Preconditions**
- M2-T001..T004 DONE; required DRs accepted.

**Implementation procedure**
1. Review prototype results against M0 contracts.
2. Update ADR-001/002/004 as accepted decisions where applicable.
3. Update traceability and risks.
4. Archive rejected alternatives and rationale.

**Tests**
- Architecture checklist; dependency graph; contract tests.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All M2 P0 decisions are explicit.
- [ ] No unresolved architecture ambiguity blocks M3.
- [ ] Rejected alternatives are preserved as evidence.

**Allowed paths**
- `docs/development/`
- `docs/decisions/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M2-T005-implementation.md`
- `docs/development/evidence/M2-T005-tests.md`
- `docs/development/evidence/M2-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M3 — DSP mathematical foundation

#### M3-T001 — Implement parameter types and stable IDs

**Priority:** P0  
**Dependencies:** M2-T005  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Turn the frozen parameter/state contract into strongly typed, validated Rust APIs.

**Preconditions**
- M2 gate PASS.

**Implementation procedure**
1. Implement stable ParamId mapping and constants.
2. Implement enums and ranges from PARAMETER_STATE_CONTRACT.
3. Implement clamp/validation semantics without hiding invalid-input diagnostics where diagnostics are needed.
4. Add exhaustive boundary tests.

**Tests**
- Unit; property; serialization-independent validation tests.

**Validation commands**
```bash
cargo test -p romueq-common --all-features
```

**Acceptance — all must be PASS**
- [ ] All IDs/ranges/enums match the contract.
- [ ] Boundary values are deterministic.
- [ ] Invalid values cannot create illegal internal state.

**Allowed paths**
- `crates/romueq-common/`
- `docs/product/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M3-T001-implementation.md`
- `docs/development/evidence/M3-T001-tests.md`
- `docs/development/evidence/M3-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M3-T002 — Implement biquad coefficient/reference math

**Priority:** P0  
**Dependencies:** M3-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement reference biquad coefficient equations for all MVP filter types and slopes.

**Preconditions**
- M3-T001 DONE.

**Implementation procedure**
1. Implement coefficient generation as pure functions.
2. Handle Bell, Low Shelf, High Shelf, High Pass, Low Pass and supported slopes.
3. Define frequency/sample-rate boundary handling.
4. Compare against independently generated/reference vectors.
5. Reject or clamp non-finite/intermediate-invalid values according to the parameter contract.

**Tests**
- Analytical/reference vectors; frequency sweeps; stability checks; extreme parameters; NaN/Inf tests.

**Validation commands**
```bash
cargo test -p romueq-dsp --all-features
```

**Acceptance — all must be PASS**
- [ ] Reference vectors match tolerance.
- [ ] Coefficients are finite for all accepted parameters.
- [ ] Stable filters satisfy the chosen stability criterion.
- [ ] No hidden allocation in coefficient calculation.

**Allowed paths**
- `crates/romueq-dsp/`
- `tests/fixtures/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M3-T002-implementation.md`
- `docs/development/evidence/M3-T002-tests.md`
- `docs/development/evidence/M3-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M3-T003 — Implement filter state/reset semantics

**Priority:** P0  
**Dependencies:** M3-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement filter runtime state, reset and preparation semantics.

**Preconditions**
- M3-T002 DONE.

**Implementation procedure**
1. Define per-channel state.
2. Implement reset to known zero/history state.
3. Implement prepare/reprepare with sample-rate and block context as required.
4. Prove processing after reset/reprepare is deterministic.

**Tests**
- Impulse; silence; reset equivalence; sample-rate transition tests; mono/stereo state isolation.

**Validation commands**
```bash
cargo test -p romueq-dsp --all-features
```

**Acceptance — all must be PASS**
- [ ] Reset produces known behavior.
- [ ] No stale history survives required reset.
- [ ] Channel states cannot cross-contaminate.

**Allowed paths**
- `crates/romueq-dsp/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M3-T003-implementation.md`
- `docs/development/evidence/M3-T003-tests.md`
- `docs/development/evidence/M3-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M3-T004 — Implement state/preset schema and migrations

**Priority:** P0  
**Dependencies:** M3-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement versioned logical state and preset serialization/migrations.

**Preconditions**
- M3-T003 DONE; ADR-006 accepted.

**Implementation procedure**
1. Define schema version 1.
2. Serialize complete logical parameter state, not DSP history.
3. Implement explicit migration dispatch.
4. Reject unsupported future versions safely.
5. Separate preset metadata from plugin/project state.
6. Create golden migration fixtures.

**Tests**
- Round-trip; malformed input; unknown fields; unsupported future version; migration chain tests.

**Validation commands**
```bash
cargo test -p romueq-common --all-features
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Version 1 round-trip is lossless within documented serialization rules.
- [ ] Unsupported future state fails safely.
- [ ] Migration is deterministic.
- [ ] DSP coefficients/history are not persisted as authoritative state.

**Allowed paths**
- `crates/romueq-common/`
- `docs/product/`
- `tests/fixtures/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M3-T004-implementation.md`
- `docs/development/evidence/M3-T004-tests.md`
- `docs/development/evidence/M3-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M3-T005 — Run M3 DSP gate

**Priority:** P0  
**Dependencies:** M3-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close DSP foundation with reference evidence.

**Preconditions**
- M3-T001..T004 DONE.

**Implementation procedure**
1. Run all DSP vectors and property tests.
2. Review numerical tolerances.
3. Freeze golden fixtures and document generation.
4. Update traceability and risk register.

**Tests**
- Complete DSP suite plus fixture reproducibility.

**Validation commands**
```bash
cargo test -p romueq-dsp --all-features
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All P0 DSP acceptance criteria PASS.
- [ ] Fixtures are versioned and reproducible.
- [ ] No unresolved stability/numerical P0 risk.

**Allowed paths**
- `docs/development/`
- `tests/fixtures/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M3-T005-implementation.md`
- `docs/development/evidence/M3-T005-tests.md`
- `docs/development/evidence/M3-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M4 — Real-time engine

#### M4-T001 — Implement allocation-free engine lifecycle

**Priority:** P0  
**Dependencies:** M3-T005  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Build the engine lifecycle with strict real-time ownership boundaries.

**Preconditions**
- M3 gate PASS; ADR-007 remains Proposed until evidence.

**Implementation procedure**
1. Implement prepare/reprepare/reset lifecycle.
2. Preallocate all process-time resources.
3. Keep mutable DSP coefficients/state on audio side.
4. Keep control-plane mutation outside process except approved lock-free transport.
5. Make failure modes explicit before audio processing starts.

**Tests**
- Allocation tracking; lifecycle stress; repeated prepare/reset; no-panic tests.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
```

**Acceptance — all must be PASS**
- [ ] process path performs zero allocations.
- [ ] No locks/I/O/syscalls/logging in hot path.
- [ ] Engine cannot process before successful prepare.
- [ ] Reset/reprepare semantics match M2 contract.

**Allowed paths**
- `crates/romueq-engine/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M4-T001-implementation.md`
- `docs/development/evidence/M4-T001-tests.md`
- `docs/development/evidence/M4-T001-evidence.md`

**STOP-THE-LINE if**
- Allocator tracking cannot prove zero allocation.
- A lock/blocking primitive appears in process.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M4-T002 — Implement block processing and variable block sizes

**Priority:** P0  
**Dependencies:** M4-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement variable-block mono/stereo processing and band cascade.

**Preconditions**
- M4-T001 DONE.

**Implementation procedure**
1. Process arbitrary block lengths allowed by prepare.
2. Handle mono and stereo.
3. Apply up to eight enabled bands in deterministic order.
4. Implement output gain and bypass path according to current contract.
5. Test in-place and non-in-place where contract allows.

**Tests**
- Block sizes 1, 2, 3, powers of two, max block; mono/stereo; silence/impulse/sine; split-block equivalence.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
```

**Acceptance — all must be PASS**
- [ ] Split processing matches whole-block processing within defined tolerance.
- [ ] Channel routing is correct.
- [ ] All eight bands can coexist.

**Allowed paths**
- `crates/romueq-engine/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M4-T002-implementation.md`
- `docs/development/evidence/M4-T002-tests.md`
- `docs/development/evidence/M4-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M4-T003 — Implement sample-accurate parameter application

**Priority:** P0  
**Dependencies:** M4-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Apply parameter events at exact block-relative sample offsets.

**Preconditions**
- M4-T002 DONE; ADR-005 accepted.

**Implementation procedure**
1. Consume sorted host events.
2. Apply event exactly at its offset.
3. Define same-sample ordering.
4. Smooth only where the parameter contract requires smoothing; do not alter event timing.
5. Test events at 0, middle and last sample.

**Tests**
- Random automation; same-offset sequences; rapid changes; block boundary events; split-block equivalence.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
```

**Acceptance — all must be PASS**
- [ ] Event at offset N affects processing starting at sample N according to documented convention.
- [ ] Same-offset sequence ordering is deterministic.
- [ ] No event is silently dropped or block-quantized.

**Allowed paths**
- `crates/romueq-engine/`
- `crates/romueq-common/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M4-T003-implementation.md`
- `docs/development/evidence/M4-T003-tests.md`
- `docs/development/evidence/M4-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M4-T004 — Implement bypass/reprepare behavior

**Priority:** P0  
**Dependencies:** M4-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement click-safe bypass and reprepare behavior under the accepted contract.

**Preconditions**
- M4-T003 DONE; ADR-008 decision outcome available.

**Implementation procedure**
1. Implement selected bypass transition behavior.
2. Measure discontinuity at entry/exit.
3. Ensure reprepare resets filter state while preserving logical parameters.
4. Ensure no old configuration is processed after successful reprepare.

**Tests**
- Bypass on/off at arbitrary offsets; impulse/transient; reprepare sample-rate changes; reset equivalence.

**Validation commands**
```bash
cargo test -p romueq-engine --all-features
```

**Acceptance — all must be PASS**
- [ ] Bypass behavior is deterministic and click-safe to the accepted threshold.
- [ ] Reprepare semantics pass.
- [ ] No stale state leaks across configurations.

**Allowed paths**
- `crates/romueq-engine/`
- `docs/decisions/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M4-T004-implementation.md`
- `docs/development/evidence/M4-T004-tests.md`
- `docs/development/evidence/M4-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M4-T005 — Run M4 real-time safety gate

**Priority:** P0  
**Dependencies:** M4-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close the real-time engine gate with hard evidence.

**Preconditions**
- M4-T001..T004 DONE.

**Implementation procedure**
1. Run allocator harness and hostile processing sequences.
2. Review source for forbidden operations.
3. Run concurrency/model tests for control path.
4. Update ADR-007 if accepted and close P0 risks.

**Tests**
- No-alloc; stress; property; lifecycle; deterministic render tests.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All RT P0 invariants PASS.
- [ ] Allocator evidence is zero during process.
- [ ] No unresolved P0 RT issue.

**Allowed paths**
- `docs/development/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M4-T005-implementation.md`
- `docs/development/evidence/M4-T005-tests.md`
- `docs/development/evidence/M4-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M5 — Analysis pipeline

#### M5-T001 — Implement analysis data path outside audio thread

**Priority:** P1  
**Dependencies:** M4-T005  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Move analysis transport fully outside the audio thread.

**Preconditions**
- M4 gate PASS.

**Implementation procedure**
1. Define bounded analysis handoff.
2. Copy/decimate only what is required, with preallocated buffers.
3. Ensure audio thread never runs FFT or GUI work.
4. Define drop/backpressure policy.

**Tests**
- Stress analysis under overload; allocation tracking on process; deterministic drop policy.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Audio thread remains RT-safe.
- [ ] Analysis overload cannot block audio.
- [ ] Dropped analysis frames are observable/documented if applicable.

**Allowed paths**
- `crates/romueq-analysis/`
- `crates/romueq-engine/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M5-T001-implementation.md`
- `docs/development/evidence/M5-T001-tests.md`
- `docs/development/evidence/M5-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M5-T002 — Implement spectrum/FFT pipeline

**Priority:** P1  
**Dependencies:** M5-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement FFT/spectrum processing with stable configuration and output semantics.

**Preconditions**
- M5-T001 DONE.

**Implementation procedure**
1. Select FFT implementation only after dependency policy check.
2. Define window, size, overlap, smoothing/averaging and frequency-bin mapping.
3. Generate reproducible test vectors.
4. Keep FFT allocations out of audio path.

**Tests**
- Sine-bin tests; impulse; noise; sample-rate changes; FFT size boundaries.

**Validation commands**
```bash
cargo test -p romueq-analysis --all-features
```

**Acceptance — all must be PASS**
- [ ] Frequency mapping is correct.
- [ ] Expected spectral peaks are within tolerance.
- [ ] Configuration changes do not corrupt buffers.

**Allowed paths**
- `crates/romueq-analysis/`
- `tests/fixtures/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M5-T002-implementation.md`
- `docs/development/evidence/M5-T002-tests.md`
- `docs/development/evidence/M5-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M5-T003 — Implement meter computation and transport

**Priority:** P1  
**Dependencies:** M5-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement meters and their non-blocking transport to presentation.

**Preconditions**
- M5-T002 DONE.

**Implementation procedure**
1. Define peak/RMS or other selected metrics.
2. Define attack/release/hold semantics.
3. Define reset and silence behavior.
4. Publish data without blocking audio.

**Tests**
- Known-level sine; silence; impulse; overload; reset; transport stress.

**Validation commands**
```bash
cargo test -p romueq-analysis --all-features
```

**Acceptance — all must be PASS**
- [ ] Meter values match reference within tolerance.
- [ ] Transport cannot block audio.
- [ ] No stale meter state after reset.

**Allowed paths**
- `crates/romueq-analysis/`
- `crates/romueq-engine/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M5-T003-implementation.md`
- `docs/development/evidence/M5-T003-tests.md`
- `docs/development/evidence/M5-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M5-T004 — Implement analysis configuration/state boundaries

**Priority:** P1  
**Dependencies:** M5-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Define analysis configuration/state boundaries and lifecycle.

**Preconditions**
- M5-T003 DONE.

**Implementation procedure**
1. Separate analysis configuration from logical EQ state unless explicitly required.
2. Define enable/disable behavior.
3. Define sample-rate/reprepare handling.
4. Define ownership and thread boundaries.

**Tests**
- Lifecycle and configuration property tests.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Analysis cannot mutate authoritative DSP state.
- [ ] Configuration changes are safe.

**Allowed paths**
- `crates/romueq-analysis/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M5-T004-implementation.md`
- `docs/development/evidence/M5-T004-tests.md`
- `docs/development/evidence/M5-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M5-T005 — Run M5 analysis gate

**Priority:** P1  
**Dependencies:** M5-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close analysis gate without introducing RT coupling.

**Preconditions**
- M5-T001..T004 DONE.

**Implementation procedure**
1. Run analysis reference suite and overload tests.
2. Review thread boundaries.
3. Update traceability/risk.

**Tests**
- Full workspace tests plus analysis fixtures.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] M5 acceptance is fully evidenced.
- [ ] No audio-thread FFT/analysis dependency remains.

**Allowed paths**
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M5-T005-implementation.md`
- `docs/development/evidence/M5-T005-tests.md`
- `docs/development/evidence/M5-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M6 — GUI and interaction model

#### M6-T001 — Freeze GUI framework and lifecycle contract

**Priority:** P0  
**Dependencies:** M5-T005  
**Decision dependencies:** DR-2026-004  
**Human decision:** YES  

**Purpose**  Freeze the GUI framework and plugin/standalone lifecycle only after prototype evidence.

**Preconditions**
- M5 gate PASS; DR-2026-004 required.

**Implementation procedure**
1. Evaluate the candidate GUI framework against embedding, resize, HiDPI, focus, input and multi-instance criteria.
2. Record build/licensing implications.
3. Freeze one framework only after decision acceptance.
4. Define GUI-to-engine parameter binding without direct DSP access.

**Tests**
- Prototype plugin embedding; standalone window; resize/HiDPI; focus; multi-instance.

**Validation commands**
```bash
cargo check --workspace
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Framework decision is explicit and accepted.
- [ ] GUI depends on contracts, not audio implementation details.

**Allowed paths**
- `crates/romueq-gui/`
- `docs/decisions/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M6-T001-implementation.md`
- `docs/development/evidence/M6-T001-tests.md`
- `docs/development/evidence/M6-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M6-T002 — Implement parameter editor and band controls

**Priority:** P1  
**Dependencies:** M6-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement the expert parameter editor and safe band controls.

**Preconditions**
- M6-T001 DONE.

**Implementation procedure**
1. Implement eight-band enable/type/frequency/gain/Q/slope controls.
2. Use stable ParamId bindings.
3. Validate/clamp at the model boundary.
4. Emit control commands; never mutate DSP state directly.
5. Handle keyboard/mouse focus and value entry.

**Tests**
- Widget/model binding; boundary values; rapid edits; reset/defaults.

**Validation commands**
```bash
cargo test -p romueq-gui --all-features
```

**Acceptance — all must be PASS**
- [ ] Every visible control maps to exactly one stable parameter.
- [ ] No GUI path bypasses validation.
- [ ] Rapid edits do not block audio.

**Allowed paths**
- `crates/romueq-gui/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M6-T002-implementation.md`
- `docs/development/evidence/M6-T002-tests.md`
- `docs/development/evidence/M6-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M6-T003 — Implement graph/spectrum presentation

**Priority:** P1  
**Dependencies:** M6-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement graph and spectrum presentation using analysis snapshots.

**Preconditions**
- M6-T002 DONE; M5 analysis API frozen.

**Implementation procedure**
1. Render EQ response from logical parameters.
2. Render spectrum snapshots.
3. Define stale/missing analysis display behavior.
4. Implement expert graph interaction only if in MVP contract.

**Tests**
- Coordinate mapping; frequency/gain boundary tests; stale snapshot handling; resize.

**Validation commands**
```bash
cargo test -p romueq-gui --all-features
```

**Acceptance — all must be PASS**
- [ ] Graph never becomes authoritative DSP state.
- [ ] Visual response is consistent with logical parameters.

**Allowed paths**
- `crates/romueq-gui/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M6-T003-implementation.md`
- `docs/development/evidence/M6-T003-tests.md`
- `docs/development/evidence/M6-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M6-T004 — Implement preset/state UI interactions

**Priority:** P1  
**Dependencies:** M6-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement preset/state UI without conflating project state and presets.

**Preconditions**
- M6-T003 DONE; ADR-006 accepted.

**Implementation procedure**
1. Implement load/save UI actions.
2. Display migration/load errors safely.
3. Keep undo/redo in adapter/UI layer.
4. Persist only supported state fields.

**Tests**
- Round-trip UI state; invalid file; future version; preset metadata.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] State UI uses schema APIs.
- [ ] Errors are recoverable and non-destructive.

**Allowed paths**
- `crates/romueq-gui/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M6-T004-implementation.md`
- `docs/development/evidence/M6-T004-tests.md`
- `docs/development/evidence/M6-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M6-T005 — Run M6 GUI gate

**Priority:** P1  
**Dependencies:** M6-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close GUI gate with interaction and lifecycle evidence.

**Preconditions**
- M6-T001..T004 DONE.

**Implementation procedure**
1. Run UI checklist on supported OSes.
2. Capture screenshots/logs where useful.
3. Update decision/traceability/risk.

**Tests**
- GUI smoke and regression suite.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] All M6 P0/P1 gate criteria pass or are explicitly deferred by contract.

**Allowed paths**
- `docs/development/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M6-T005-implementation.md`
- `docs/development/evidence/M6-T005-tests.md`
- `docs/development/evidence/M6-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M7 — Plugin adapters

#### M7-T001 — Implement CLAP adapter

**Priority:** P0  
**Dependencies:** M6-T005  
**Decision dependencies:** DR-2026-003  
**Human decision:** YES  

**Purpose**  Implement the selected CLAP adapter as a thin host boundary.

**Preconditions**
- M6 gate PASS; DR-2026-003 accepted.

**Implementation procedure**
1. Map lifecycle to engine prepare/process/reset.
2. Map stable parameters.
3. Preserve sample offsets.
4. Implement state save/load on host-approved thread.
5. Implement editor binding through GUI contract.

**Tests**
- CLAP validation; lifecycle; automation; state; multiple instances; variable blocks.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] No CLAP types leak into core crates.
- [ ] Automation offsets survive adapter mapping.
- [ ] State round-trips.

**Allowed paths**
- `crates/romueq-plugin/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M7-T001-implementation.md`
- `docs/development/evidence/M7-T001-tests.md`
- `docs/development/evidence/M7-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M7-T002 — Implement VST3 adapter

**Priority:** P0  
**Dependencies:** M7-T001  
**Decision dependencies:** DR-2026-003  
**Human decision:** YES  

**Purpose**  Implement the selected VST3 adapter as a thin host boundary.

**Preconditions**
- M7-T001 DONE; DR-2026-003 accepted.

**Implementation procedure**
1. Map lifecycle and process buffers.
2. Map parameter queues with sample offsets.
3. Implement state serialization bridge.
4. Handle activation/deactivation and editor lifecycle.

**Tests**
- VST3 validator/host tests; automation; state; block sizes; activate/deactivate.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] VST3 host data never bypasses engine contracts.
- [ ] Parameter offsets preserved.
- [ ] State mapping is deterministic.

**Allowed paths**
- `crates/romueq-plugin/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M7-T002-implementation.md`
- `docs/development/evidence/M7-T002-tests.md`
- `docs/development/evidence/M7-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M7-T003 — Implement plugin state/automation mapping

**Priority:** P0  
**Dependencies:** M7-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Make plugin parameter/state mapping lossless and version-safe.

**Preconditions**
- M7-T002 DONE.

**Implementation procedure**
1. Create exhaustive host-ID ↔ ParamId mapping table.
2. Test normalized/physical conversions.
3. Test state version migration through both adapters.
4. Define unsupported parameter behavior.

**Tests**
- Round-trip all parameters; automation; state fixtures; malformed host values.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Every MVP parameter has one stable mapping.
- [ ] No parameter drift between CLAP/VST3.
- [ ] State is schema-versioned.

**Allowed paths**
- `crates/romueq-plugin/`
- `crates/romueq-common/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M7-T003-implementation.md`
- `docs/development/evidence/M7-T003-tests.md`
- `docs/development/evidence/M7-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M7-T004 — Validate plugin lifecycle and editor embedding

**Priority:** P0  
**Dependencies:** M7-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Validate plugin lifecycle, editor embedding and host-owned resources.

**Preconditions**
- M7-T003 DONE.

**Implementation procedure**
1. Test scan/instantiate/activate/process/deactivate/destroy.
2. Test editor open/close/resize/focus.
3. Test multiple instances.
4. Test offline/non-realtime processing path.

**Tests**
- Host matrix lifecycle tests.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] No lifecycle sequence causes panic/hang/state corruption.
- [ ] Multiple instances remain isolated.

**Allowed paths**
- `crates/romueq-plugin/`
- `docs/compatibility/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M7-T004-implementation.md`
- `docs/development/evidence/M7-T004-tests.md`
- `docs/development/evidence/M7-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M7-T005 — Run M7 plugin gate

**Priority:** P0  
**Dependencies:** M7-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close plugin gate with validator and host evidence.

**Preconditions**
- M7-T001..T004 DONE.

**Implementation procedure**
1. Run official validator(s) where applicable.
2. Run selected hosts.
3. Archive binaries, logs and version information.
4. Update compatibility matrix without claiming untested hosts.

**Tests**
- CLAP/VST3 validation; state/automation; lifecycle; GUI.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Both plugin formats meet MVP acceptance.
- [ ] All known P0 failures are resolved.

**Allowed paths**
- `docs/development/`
- `docs/compatibility/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M7-T005-implementation.md`
- `docs/development/evidence/M7-T005-tests.md`
- `docs/development/evidence/M7-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M8 — Standalone + CLI

#### M8-T001 — Implement standalone host wrapper

**Priority:** P1  
**Dependencies:** M7-T005  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Build a standalone wrapper using the same engine and GUI contracts.

**Preconditions**
- M7 gate PASS.

**Implementation procedure**
1. Create standalone lifecycle.
2. Reuse GUI without special DSP paths.
3. Implement audio device selection/configuration outside engine.
4. Handle device changes via reprepare.

**Tests**
- Open/close; device changes; sample-rate/block-size; state; GUI.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Standalone audio path uses the same engine.
- [ ] Device changes cannot corrupt state.

**Allowed paths**
- `crates/romueq-standalone/`
- `crates/romueq-gui/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M8-T001-implementation.md`
- `docs/development/evidence/M8-T001-tests.md`
- `docs/development/evidence/M8-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M8-T002 — Implement CLI offline renderer

**Priority:** P1  
**Dependencies:** M8-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Implement a deterministic CLI offline renderer.

**Preconditions**
- M8-T001 DONE.

**Implementation procedure**
1. Define input/output format support for MVP.
2. Parse versioned state/preset.
3. Render through the same engine.
4. Define error exit codes.
5. Do not duplicate DSP math.

**Tests**
- Golden renders; malformed state; short/long input; mono/stereo; sample-rate.

**Validation commands**
```bash
cargo test -p romueq-cli --all-features
```

**Acceptance — all must be PASS**
- [ ] CLI uses engine output path.
- [ ] Same input/state yields repeatable output within documented tolerance.

**Allowed paths**
- `crates/romueq-cli/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M8-T002-implementation.md`
- `docs/development/evidence/M8-T002-tests.md`
- `docs/development/evidence/M8-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M8-T003 — Implement offline reference comparisons

**Priority:** P1  
**Dependencies:** M8-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Create offline reference comparisons between CLI and engine fixtures.

**Preconditions**
- M8-T002 DONE.

**Implementation procedure**
1. Generate canonical inputs.
2. Render with independent invocation.
3. Compare samples and summary metrics.
4. Store fixture metadata and tolerance.

**Tests**
- Impulse/sine/sweep/noise; multiple rates/block sizes; all filter types.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] No unexplained render divergence.
- [ ] Fixture generation is reproducible.

**Allowed paths**
- `tests/fixtures/`
- `crates/romueq-cli/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M8-T003-implementation.md`
- `docs/development/evidence/M8-T003-tests.md`
- `docs/development/evidence/M8-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M8-T004 — Implement packaging scripts

**Priority:** P1  
**Dependencies:** M8-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Automate packaging of plugin, standalone and CLI artifacts.

**Preconditions**
- M8-T003 DONE.

**Implementation procedure**
1. Define versioned artifact names.
2. Package only approved formats/platforms.
3. Include licenses and notices.
4. Generate checksums.
5. Make packaging fail on missing artifact.

**Tests**
- Package-from-clean-tree; manifest inspection; checksum verification.

**Validation commands**
```bash
cargo build --workspace --release
```

**Acceptance — all must be PASS**
- [ ] Packages are complete, identifiable and reproducible from documented inputs.

**Allowed paths**
- `scripts/`
- `docs/development/`
- `README.md`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M8-T004-implementation.md`
- `docs/development/evidence/M8-T004-tests.md`
- `docs/development/evidence/M8-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M8-T005 — Run M8 standalone/CLI gate

**Priority:** P1  
**Dependencies:** M8-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close standalone/CLI gate.

**Preconditions**
- M8-T001..T004 DONE.

**Implementation procedure**
1. Run clean-state renders and standalone smoke.
2. Archive artifacts/evidence.
3. Update traceability.

**Tests**
- Full M8 suite.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Standalone and CLI meet their acceptance contracts.

**Allowed paths**
- `docs/development/`
- `docs/compatibility/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M8-T005-implementation.md`
- `docs/development/evidence/M8-T005-tests.md`
- `docs/development/evidence/M8-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M9 — Compatibility and hostile-host testing

#### M9-T001 — Freeze exact host compatibility matrix

**Priority:** P0  
**Dependencies:** M8-T005  
**Decision dependencies:** DR-2026-005  
**Human decision:** YES  

**Purpose**  Freeze the exact DAW/host/version compatibility matrix before execution.

**Preconditions**
- M8 gate PASS; DR-2026-005 required.

**Implementation procedure**
1. Select representative hosts for each supported OS/format.
2. Record exact versions, architecture and test environment.
3. Define pass/fail cases per host.
4. Do not infer host compatibility from another version.

**Tests**
- Matrix review and reproducibility check.

**Validation commands**
```bash
python3 scripts/validate_m05.py
```

**Acceptance — all must be PASS**
- [ ] Exact matrix is frozen and accepted.
- [ ] Each test case has a defined expected result.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M9-T001-implementation.md`
- `docs/development/evidence/M9-T001-tests.md`
- `docs/development/evidence/M9-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M9-T002 — Execute scan/instantiate/lifecycle matrix

**Priority:** P0  
**Dependencies:** M9-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Execute scan, instantiate and lifecycle compatibility matrix.

**Preconditions**
- M9-T001 DONE.

**Implementation procedure**
1. Install clean artifacts.
2. Test scan/discovery.
3. Instantiate multiple instances.
4. Activate/deactivate/destroy.
5. Open/close editor.

**Tests**
- Every host/version in matrix.

**Validation commands**
```bash
Host-specific commands documented in evidence; cargo tests as regression.
```

**Acceptance — all must be PASS**
- [ ] All mandatory hosts scan and instantiate.
- [ ] No crash/hang in lifecycle.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M9-T002-implementation.md`
- `docs/development/evidence/M9-T002-tests.md`
- `docs/development/evidence/M9-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M9-T003 — Execute automation/state/bypass matrix

**Priority:** P0  
**Dependencies:** M9-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Execute automation, state, bypass and parameter compatibility matrix.

**Preconditions**
- M9-T002 DONE.

**Implementation procedure**
1. Automate every parameter class.
2. Test same-offset/multi-offset events.
3. Save/load projects/presets.
4. Toggle bypass during audio.
5. Reload state after host restart.

**Tests**
- Host automation/state matrix.

**Validation commands**
```bash
Host-specific evidence plus regression tests.
```

**Acceptance — all must be PASS**
- [ ] All mandatory host cases pass.
- [ ] No parameter/state corruption.
- [ ] Bypass remains within contract.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M9-T003-implementation.md`
- `docs/development/evidence/M9-T003-tests.md`
- `docs/development/evidence/M9-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M9-T004 — Execute hostile block/rate/offline scenarios

**Priority:** P0  
**Dependencies:** M9-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Exercise hostile block-size, sample-rate and offline-render scenarios.

**Preconditions**
- M9-T003 DONE.

**Implementation procedure**
1. Use minimum/maximum supported blocks.
2. Change sample rate across sessions.
3. Run offline/bounce modes.
4. Test transport stops/starts and rapid lifecycle sequences.

**Tests**
- Host hostile scenarios.

**Validation commands**
```bash
Host-specific evidence plus cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] No P0 failure under hostile cases.
- [ ] Offline rendering remains deterministic within documented tolerance.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M9-T004-implementation.md`
- `docs/development/evidence/M9-T004-tests.md`
- `docs/development/evidence/M9-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M9-T005 — Run M9 compatibility gate

**Priority:** P0  
**Dependencies:** M9-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close compatibility gate with explicit pass/fail per host.

**Preconditions**
- M9-T001..T004 DONE.

**Implementation procedure**
1. Summarize each host/version.
2. Quarantine nothing silently.
3. Create remediation tasks for failures.
4. Update supported-host claims.

**Tests**
- Matrix completeness review.

**Validation commands**
```bash
python3 scripts/validate_m05.py
```

**Acceptance — all must be PASS**
- [ ] Every mandatory host is PASS or formally excluded before release.
- [ ] No untested compatibility claim remains.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M9-T005-implementation.md`
- `docs/development/evidence/M9-T005-tests.md`
- `docs/development/evidence/M9-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M10 — Performance, reliability, hardening

#### M10-T001 — Freeze performance reference environment

**Priority:** P0  
**Dependencies:** M9-T005  
**Decision dependencies:** DR-2026-006  
**Human decision:** YES  

**Purpose**  Freeze the performance reference environment and measurement protocol.

**Preconditions**
- M9 gate PASS; DR-2026-006 required.

**Implementation procedure**
1. Define CPU model, OS, power mode, sample rate, buffer size, instance count and measurement duration.
2. Define warm-up and sampling rules.
3. Define memory/allocation metrics.
4. Freeze baseline commit.

**Tests**
- Protocol repeatability check.

**Validation commands**
```bash
cargo bench --workspace
```

**Acceptance — all must be PASS**
- [ ] Reference environment and benchmark protocol are immutable for comparison unless versioned.

**Allowed paths**
- `docs/development/`
- `benches/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M10-T001-implementation.md`
- `docs/development/evidence/M10-T001-tests.md`
- `docs/development/evidence/M10-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M10-T002 — Run CPU/memory/allocation benchmarks

**Priority:** P0  
**Dependencies:** M10-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Measure CPU, memory and allocation behavior against budget.

**Preconditions**
- M10-T001 DONE.

**Implementation procedure**
1. Benchmark mono/stereo, 0/8 active bands, automation, analysis enabled/disabled.
2. Measure process time distribution, not only average.
3. Run allocation harness.
4. Compare against PERFORMANCE_BUDGET.

**Tests**
- Repeated benchmark runs; allocation count; worst-case block.

**Validation commands**
```bash
cargo bench --workspace
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Performance target is met or deviation is explicitly accepted by decision.
- [ ] Audio process allocations remain zero.
- [ ] No unexplained regression from baseline.

**Allowed paths**
- `docs/development/`
- `benches/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M10-T002-implementation.md`
- `docs/development/evidence/M10-T002-tests.md`
- `docs/development/evidence/M10-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M10-T003 — Run fuzz/property/stress suite

**Priority:** P0  
**Dependencies:** M10-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Run fuzzing, property and long-duration stress validation.

**Preconditions**
- M10-T002 DONE.

**Implementation procedure**
1. Fuzz state/parameter/event decoders.
2. Run long automation and lifecycle stress.
3. Run repeated render comparisons.
4. Record seeds and minimized failures.

**Tests**
- Fuzz/property/stress suite.

**Validation commands**
```bash
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] No panic, corruption, hang or unbounded resource behavior in release paths.
- [ ] Failures are reproducible from stored seeds.

**Allowed paths**
- `fuzz/`
- `tests/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M10-T003-implementation.md`
- `docs/development/evidence/M10-T003-tests.md`
- `docs/development/evidence/M10-T003-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M10-T004 — Run security/supply-chain/reproducibility checks

**Priority:** P0  
**Dependencies:** M10-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Verify security, supply chain and reproducibility of release builds.

**Preconditions**
- M10-T003 DONE.

**Implementation procedure**
1. Run dependency/advisory/license/source audits.
2. Build from clean checkout.
3. Compare artifact hashes where reproducibility is promised.
4. Verify embedded versions/licenses.
5. Check package contents for accidental files/secrets.

**Tests**
- Clean build; audit; package inspection.

**Validation commands**
```bash
cargo deny check
cargo build --workspace --release
```

**Acceptance — all must be PASS**
- [ ] No unapproved dependency risk.
- [ ] Release build is reproducible to the documented degree.
- [ ] No secret/debug artifact leakage.

**Allowed paths**
- `deny.toml`
- `.github/`
- `scripts/`
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M10-T004-implementation.md`
- `docs/development/evidence/M10-T004-tests.md`
- `docs/development/evidence/M10-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M10-T005 — Run M10 hardening gate

**Priority:** P0  
**Dependencies:** M10-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Close hardening gate with zero unresolved release-blocking P0 issues.

**Preconditions**
- M10-T001..T004 DONE.

**Implementation procedure**
1. Run complete regression suite.
2. Review risk register.
3. Verify all P0 requirements have evidence.
4. Freeze release candidate baseline.

**Tests**
- Full test, benchmark, fuzz, audit and reproducibility suite.

**Validation commands**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

**Acceptance — all must be PASS**
- [ ] M10 PASS.
- [ ] Release candidate baseline is tagged internally.
- [ ] All P0 risks are closed or explicitly accepted by required authority.

**Allowed paths**
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M10-T005-implementation.md`
- `docs/development/evidence/M10-T005-tests.md`
- `docs/development/evidence/M10-T005-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

### M11 — Release candidate

#### M11-T001 — Assemble release artifacts

**Priority:** P0  
**Dependencies:** M10-T005  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Assemble final release artifacts from the frozen candidate.

**Preconditions**
- M10 gate PASS.

**Implementation procedure**
1. Build all approved platforms/formats.
2. Package binaries, licenses, release notes and documentation.
3. Generate checksums and manifests.
4. Record source revision/toolchain/dependency lock state.

**Tests**
- Artifact completeness and checksum tests.

**Validation commands**
```bash
cargo build --workspace --release
```

**Acceptance — all must be PASS**
- [ ] Every promised artifact exists and matches the matrix.
- [ ] Packages contain required notices.

**Allowed paths**
- `scripts/`
- `docs/development/`
- `README.md`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M11-T001-implementation.md`
- `docs/development/evidence/M11-T001-tests.md`
- `docs/development/evidence/M11-T001-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M11-T002 — Run clean-machine smoke tests

**Priority:** P0  
**Dependencies:** M11-T001  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Prove installation and startup on clean machines/environments.

**Preconditions**
- M11-T001 DONE.

**Implementation procedure**
1. Use clean VM/machine snapshots.
2. Install each relevant artifact.
3. Scan/launch plugin and standalone.
4. Run smoke audio path.
5. Uninstall and verify cleanup where applicable.

**Tests**
- Clean-machine smoke per platform.

**Validation commands**
```bash
Platform-specific evidence commands documented in report.
```

**Acceptance — all must be PASS**
- [ ] No installation/startup blocker.
- [ ] Plugin discovery and standalone launch work on supported environments.

**Allowed paths**
- `docs/compatibility/`
- `docs/development/evidence/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M11-T002-implementation.md`
- `docs/development/evidence/M11-T002-tests.md`
- `docs/development/evidence/M11-T002-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M11-T003 — Run signing/notarization/distribution checks

**Priority:** P0  
**Dependencies:** M11-T002  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Perform final signing, notarization and distribution checks where applicable.

**Preconditions**
- M11-T002 DONE.

**Implementation procedure**
1. Sign artifacts using release credentials outside source control.
2. Verify signatures.
3. Run notarization/platform distribution checks where applicable.
4. Ensure unsigned test artifacts are not mislabeled as release artifacts.

**Tests**
- Signature verification; package integrity; platform policy checks.

**Validation commands**
```bash
Platform-specific signing verification commands documented in evidence.
```

**Acceptance — all must be PASS**
- [ ] All applicable artifacts are correctly signed/notarized.
- [ ] No credential material enters repository or evidence.

**Allowed paths**
- `scripts/`
- `docs/development/`
- `release/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M11-T003-implementation.md`
- `docs/development/evidence/M11-T003-tests.md`
- `docs/development/evidence/M11-T003-evidence.md`

**STOP-THE-LINE if**
- Signing requires credentials or external approval unavailable to the executor.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M11-T004 — Publish release evidence and traceability snapshot

**Priority:** P0  
**Dependencies:** M11-T003  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Publish the final evidence and traceability snapshot.

**Preconditions**
- M11-T003 DONE.

**Implementation procedure**
1. Freeze task statuses.
2. Generate final traceability matrix.
3. Archive test/host/benchmark/security evidence.
4. Record known limitations and deferred features.
5. Verify all evidence links resolve.

**Tests**
- Evidence index completeness; traceability consistency.

**Validation commands**
```bash
python3 scripts/validate_m05.py
cargo test --workspace --all-features
```

**Acceptance — all must be PASS**
- [ ] Every release claim maps to evidence.
- [ ] No DONE task lacks required evidence.
- [ ] Deferred scope is explicit.

**Allowed paths**
- `docs/development/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M11-T004-implementation.md`
- `docs/development/evidence/M11-T004-tests.md`
- `docs/development/evidence/M11-T004-evidence.md`

**STOP-THE-LINE if**
- Required behavior cannot be demonstrated.
- A P0 contract would need to change.
- A new dependency or unsafe boundary is required without approval.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

#### M11-T005 — Run final 99% release gate

**Priority:** P0  
**Dependencies:** M11-T004  
**Decision dependencies:** none  
**Human decision:** NO  

**Purpose**  Execute the final 99% release gate without averaging away a P0 failure.

**Preconditions**
- M11-T001..T004 DONE.

**Implementation procedure**
1. Run mandatory gate checklist.
2. Calculate evidence score using documented weights.
3. Verify every mandatory P0 gate is PASS.
4. Verify legal/name clearance status.
5. Issue PASS only if score >=99% and all mandatory gates pass.

**Tests**
- Final regression and evidence audit.

**Validation commands**
```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo deny check
```

**Acceptance — all must be PASS**
- [ ] Weighted confidence score >=99%.
- [ ] All mandatory gates PASS.
- [ ] No unresolved P0 issue.
- [ ] Release name/legal gate is satisfied before public distribution.

**Allowed paths**
- `docs/development/`
- `docs/branding/`

**Forbidden paths**
- `.git/`
- `target/`

**Evidence**
- `docs/development/evidence/M11-T005-implementation.md`
- `docs/development/evidence/M11-T005-tests.md`
- `docs/development/evidence/M11-T005-evidence.md`

**STOP-THE-LINE if**
- Score <99%.
- Any P0 gate FAIL/BLOCKED.
- Formal name/trademark clearance remains open.

**Rollback / failure handling**
Revert only this task's changes; preserve prior passing evidence and return the task to BLOCKED or TESTING as appropriate.

**Definition of DONE**
- Implementation complete
- Tests PASS
- Acceptance criteria PASS
- Evidence complete
- Traceability/risk/state updated
- No unresolved stop condition

