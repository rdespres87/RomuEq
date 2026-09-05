[
  {
    "id": "M0.5-T001",
    "title": "Establish execution-state and task registry",
    "status": "READY",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0-T005"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Create and maintain task state files using the standard schema.",
      "Verify dependency ordering and status vocabulary."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "Every task has a unique ID, status, priority and dependency list.",
      "A deterministic first READY task can be selected."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T002",
    "title": "Install document precedence and stop-the-line rules",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T001"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Publish precedence rules.",
      "Add explicit P0 stop conditions to the AI protocol."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "Conflicting instructions have a defined resolution path.",
      "P0 ambiguity causes BLOCKED state."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T003",
    "title": "Create Decision Request register",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T002"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Register all known unresolved P0 decisions.",
      "Link decision dependencies to affected tasks."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "No known P0 decision remains implicit."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T004",
    "title": "Create milestone gate procedure",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T003"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Define PASS/FAIL/BLOCKED and automatic failure conditions.",
      "Define evidence and confidence scoring."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "Every milestone has a reproducible gate procedure."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T005",
    "title": "Create test/evidence/traceability system",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T004"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Publish test pyramid and evidence requirements.",
      "Create initial requirement-to-test mappings."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "P0 requirements have verification paths and evidence expectations."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T006",
    "title": "Create compatibility/performance/release control documents",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T005"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Publish version-free planning matrices.",
      "Define measurement and release evidence protocols."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "No release claim relies on undefined host versions or undocumented performance measurements."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T007",
    "title": "Convert M0-M11 roadmap into executable task registry",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T006"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Create atomic tasks with dependencies, paths, tests and acceptance criteria.",
      "Link task families to milestones and gates."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "All milestones have executable tasks.",
      "No task requires silent architecture decisions."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0.5-T008",
    "title": "Run M0.5 consistency audit and close gate package",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M0.5",
    "deps": [
      "M0.5-T007"
    ],
    "decisions": [],
    "allowed": [
      "docs/development/",
      "docs/development/tasks/"
    ],
    "forbidden": [
      "src/",
      "crates/"
    ],
    "inputs": [
      "Existing DEVELOPMENT_PLAN.md and ADR set"
    ],
    "steps": [
      "Check unique IDs, dependency references, required fields and document links.",
      "Record gate result."
    ],
    "tests": [
      "Markdown/schema consistency checks"
    ],
    "commands": [
      "python3 scripts/validate_m05.py"
    ],
    "acceptance": [
      "Audit passes with zero orphan dependencies and zero missing required fields."
    ],
    "evidence": [
      "docs/development/EXECUTION_STATE.md"
    ],
    "gate": "M0.5",
    "human": "NO",
    "rollback": "Revert only files created by this task; preserve previous plan."
  },
  {
    "id": "M0-T001",
    "title": "Freeze product scope and MVP contract",
    "status": "DONE",
    "priority": "P0",
    "milestone": "M0",
    "deps": [],
    "decisions": [
      "DR-2026-001"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "docs/product/MVP_PRODUCT_CONTRACT.md",
      "docs/development/evidence/M0-T001-IMPLEMENTATION-REPORT-2026-09-03.md"
    ],
    "gate": "M0",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M0-T002",
    "title": "Freeze platform and format targets",
    "status": "DONE",
    "priority": "P0",
    "milestone": "M0",
    "deps": [
      "M0-T001"
    ],
    "decisions": [
      "DR-2026-005"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M0",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M0-T003",
    "title": "Freeze parameter/state contract",
    "status": "DONE",
    "priority": "P0",
    "milestone": "M0",
    "deps": [
      "M0-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M0",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M0-T004",
    "title": "Freeze licensing and dependency policy",
    "status": "DONE",
    "priority": "P0",
    "milestone": "M0",
    "deps": [
      "M0-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M0",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M0-T005",
    "title": "Run M0 product-contract gate",
    "status": "DONE",
    "priority": "P0",
    "milestone": "M0",
    "deps": [
      "M0-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "docs/development/gates/M0-GATE.md",
      "docs/development/gates/M0-GATE-REGISTER.md",
      "docs/development/evidence/M0-T005-GATE-EXECUTION-2026-09-04.md"
    ],
    "gate": "M0",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M1-T001",
    "title": "Pin Rust toolchain and workspace policy",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M1",
    "deps": [
      "M0.5-T008"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M1",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M1-T002",
    "title": "Establish CI validation ladder",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M1",
    "deps": [
      "M1-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M1",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M1-T003",
    "title": "Establish dependency/security checks",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M1",
    "deps": [
      "M1-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M1",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M1-T004",
    "title": "Establish cross-platform build matrix",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M1",
    "deps": [
      "M1-T003"
    ],
    "decisions": [
      "DR-2026-005"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M1",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M1-T005",
    "title": "Run M1 repository gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M1",
    "deps": [
      "M1-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M1",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M2-T001",
    "title": "Prototype bounded parameter transport",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M2",
    "deps": [
      "M1-T005"
    ],
    "decisions": [
      "DR-2026-002"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M2",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M2-T002",
    "title": "Prototype host automation event ingestion",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M2",
    "deps": [
      "M2-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M2",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M2-T003",
    "title": "Prototype engine/plugin boundary",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M2",
    "deps": [
      "M2-T002"
    ],
    "decisions": [
      "DR-2026-003"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M2",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M2-T004",
    "title": "Prototype audio buffer and lifecycle contract",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M2",
    "deps": [
      "M2-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M2",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M2-T005",
    "title": "Run M2 architecture evidence gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M2",
    "deps": [
      "M2-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M2",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M3-T001",
    "title": "Implement parameter types and stable IDs",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M3",
    "deps": [
      "M2-T005"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M3",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M3-T002",
    "title": "Implement biquad coefficient/reference math",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M3",
    "deps": [
      "M3-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M3",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M3-T003",
    "title": "Implement filter state/reset semantics",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M3",
    "deps": [
      "M3-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M3",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M3-T004",
    "title": "Implement state/preset schema and migrations",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M3",
    "deps": [
      "M3-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M3",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M3-T005",
    "title": "Run M3 DSP gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M3",
    "deps": [
      "M3-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M3",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M4-T001",
    "title": "Implement allocation-free engine lifecycle",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M4",
    "deps": [
      "M3-T005"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M4",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M4-T002",
    "title": "Implement block processing and variable block sizes",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M4",
    "deps": [
      "M4-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M4",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M4-T003",
    "title": "Implement sample-accurate parameter application",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M4",
    "deps": [
      "M4-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M4",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M4-T004",
    "title": "Implement bypass/reprepare behavior",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M4",
    "deps": [
      "M4-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M4",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M4-T005",
    "title": "Run M4 real-time safety gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M4",
    "deps": [
      "M4-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M4",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M5-T001",
    "title": "Implement analysis data path outside audio thread",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M5",
    "deps": [
      "M4-T005"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M5",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M5-T002",
    "title": "Implement spectrum/FFT pipeline",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M5",
    "deps": [
      "M5-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M5",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M5-T003",
    "title": "Implement meter computation and transport",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M5",
    "deps": [
      "M5-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M5",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M5-T004",
    "title": "Implement analysis configuration/state boundaries",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M5",
    "deps": [
      "M5-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M5",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M5-T005",
    "title": "Run M5 analysis gate",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M5",
    "deps": [
      "M5-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M5",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M6-T001",
    "title": "Freeze GUI framework and lifecycle contract",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M6",
    "deps": [
      "M5-T005"
    ],
    "decisions": [
      "DR-2026-004"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M6",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M6-T002",
    "title": "Implement parameter editor and band controls",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M6",
    "deps": [
      "M6-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M6",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M6-T003",
    "title": "Implement graph/spectrum presentation",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M6",
    "deps": [
      "M6-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M6",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M6-T004",
    "title": "Implement preset/state UI interactions",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M6",
    "deps": [
      "M6-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M6",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M6-T005",
    "title": "Run M6 GUI gate",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M6",
    "deps": [
      "M6-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M6",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M7-T001",
    "title": "Implement CLAP adapter",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M7",
    "deps": [
      "M6-T005"
    ],
    "decisions": [
      "DR-2026-003"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M7",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M7-T002",
    "title": "Implement VST3 adapter",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M7",
    "deps": [
      "M7-T001"
    ],
    "decisions": [
      "DR-2026-003"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M7",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M7-T003",
    "title": "Implement plugin state/automation mapping",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M7",
    "deps": [
      "M7-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M7",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M7-T004",
    "title": "Validate plugin lifecycle and editor embedding",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M7",
    "deps": [
      "M7-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M7",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M7-T005",
    "title": "Run M7 plugin gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M7",
    "deps": [
      "M7-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M7",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M8-T001",
    "title": "Implement standalone host wrapper",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M8",
    "deps": [
      "M7-T005"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M8",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M8-T002",
    "title": "Implement CLI offline renderer",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M8",
    "deps": [
      "M8-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M8",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M8-T003",
    "title": "Implement offline reference comparisons",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M8",
    "deps": [
      "M8-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M8",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M8-T004",
    "title": "Implement packaging scripts",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M8",
    "deps": [
      "M8-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M8",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M8-T005",
    "title": "Run M8 standalone/CLI gate",
    "status": "BLOCKED",
    "priority": "P1",
    "milestone": "M8",
    "deps": [
      "M8-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M8",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M9-T001",
    "title": "Freeze exact host compatibility matrix",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M9",
    "deps": [
      "M8-T005"
    ],
    "decisions": [
      "DR-2026-005"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M9",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M9-T002",
    "title": "Execute scan/instantiate/lifecycle matrix",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M9",
    "deps": [
      "M9-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M9",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M9-T003",
    "title": "Execute automation/state/bypass matrix",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M9",
    "deps": [
      "M9-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M9",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M9-T004",
    "title": "Execute hostile block/rate/offline scenarios",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M9",
    "deps": [
      "M9-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M9",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M9-T005",
    "title": "Run M9 compatibility gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M9",
    "deps": [
      "M9-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M9",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M10-T001",
    "title": "Freeze performance reference environment",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M10",
    "deps": [
      "M9-T005"
    ],
    "decisions": [
      "DR-2026-006"
    ],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M10",
    "human": "YES",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M10-T002",
    "title": "Run CPU/memory/allocation benchmarks",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M10",
    "deps": [
      "M10-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M10",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M10-T003",
    "title": "Run fuzz/property/stress suite",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M10",
    "deps": [
      "M10-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M10",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M10-T004",
    "title": "Run security/supply-chain/reproducibility checks",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M10",
    "deps": [
      "M10-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M10",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M10-T005",
    "title": "Run M10 hardening gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M10",
    "deps": [
      "M10-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M10",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M11-T001",
    "title": "Assemble release artifacts",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M11",
    "deps": [
      "M10-T005"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M11",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M11-T002",
    "title": "Run clean-machine smoke tests",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M11",
    "deps": [
      "M11-T001"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M11",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M11-T003",
    "title": "Run signing/notarization/distribution checks",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M11",
    "deps": [
      "M11-T002"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M11",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M11-T004",
    "title": "Publish release evidence and traceability snapshot",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M11",
    "deps": [
      "M11-T003"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M11",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  },
  {
    "id": "M11-T005",
    "title": "Run final 99% release gate",
    "status": "BLOCKED",
    "priority": "P0",
    "milestone": "M11",
    "deps": [
      "M11-T004"
    ],
    "decisions": [],
    "allowed": [
      "crates/",
      "src/",
      "docs/",
      ".github/"
    ],
    "forbidden": [
      ".git/",
      "target/"
    ],
    "inputs": [
      "Master plan, prior milestone outputs, linked ADRs"
    ],
    "steps": [
      "Implement only the named deliverable.",
      "Add/update tests before acceptance.",
      "Update traceability and risk entries when required."
    ],
    "tests": [
      "Task-specific unit/integration tests",
      "Relevant regression fixtures"
    ],
    "commands": [
      "cargo fmt --all -- --check",
      "cargo clippy --workspace --all-targets --all-features -- -D warnings",
      "cargo test --workspace --all-features"
    ],
    "acceptance": [
      "All stated acceptance checks pass.",
      "No out-of-scope architectural change is introduced."
    ],
    "evidence": [
      "Task implementation report",
      "Test report",
      "Evidence index"
    ],
    "gate": "M11",
    "human": "NO",
    "rollback": "If acceptance fails, return to TESTING or create a DR; do not widen scope."
  }
]
