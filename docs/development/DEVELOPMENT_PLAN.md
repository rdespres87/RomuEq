# RomuEq Development Plan

> **Document status:** Development master plan / pre-production  
> **Project name:** RomuEq — provisional until final trademark, domain and product-name clearance  
> **Target confidence before production release:** **≥ 99% readiness confidence**
>
> This document is intentionally conservative. The objective is not to start coding
> quickly; it is to reduce architectural, DSP, real-time, compatibility and
> distribution risk before each irreversible decision.

---

## 0. How to interpret the 99% target

A literal mathematical 99% certainty is impossible for a software/audio product.
Here, **99% confidence means evidence-backed readiness**:

- every P0 architectural assumption has a reproducible proof or prototype;
- every critical real-time invariant is tested;
- DSP behavior is validated against analytical/reference results;
- host/plugin lifecycle is validated on representative DAWs;
- state and automation are backward-compatible and migration-tested;
- CI is green on all supported targets;
- performance has measured headroom rather than relying on estimates;
- release artifacts are reproducible and independently smoke-tested;
- unresolved issues are explicitly documented and none are release-blocking.

### Confidence rule

A milestone cannot be declared complete because the code "looks correct".

It requires:

1. **Implementation**
2. **Tests**
3. **Evidence**
4. **Review**
5. **CI validation**
6. **Explicit exit criteria**
7. **Known-risk register update**

If evidence is missing, the milestone remains **OPEN**.

---

# 1. Non-negotiable project rules

## 1.1 Real-time audio contract

The audio hot path must be:

- allocation-free;
- non-blocking;
- I/O-free;
- syscall-free;
- GUI-free;
- host-call-free except where explicitly permitted by the host API contract;
- deterministic within documented numerical tolerances;
- free of intentional panic paths.

No PR may weaken these guarantees without an ADR and dedicated evidence.

## 1.2 Rust safety contract

Default rule:

> **Safe Rust first. `unsafe` is exceptional, isolated and justified.**

Current core crates use:

```rust
#![forbid(unsafe_code)]
```

If an external plugin/API binding later requires `unsafe`:

- isolate it in the smallest possible adapter module/crate;
- document every `unsafe` block with its safety invariant;
- add a test that exercises the invariant;
- never expose unsafe requirements to `romueq-dsp`;
- require review from the architecture owner;
- add the decision to an ADR if it affects architecture.

## 1.3 Dependency rule

Do not add a dependency simply because it is popular.

For every non-trivial dependency, record:

- purpose;
- license;
- maintenance status;
- transitive dependency impact;
- runtime impact;
- real-time impact;
- platform support;
- build impact;
- security history;
- alternatives considered.

Run:

```bash
cargo deny check
```

before merging dependency changes.

## 1.4 API stability rule

The following are public contracts and must not change casually:

- parameter IDs;
- parameter ranges;
- state schema;
- preset schema;
- plugin identifiers;
- audio buffer semantics;
- automation semantics;
- latency reporting;
- channel-layout semantics.

Any breaking change requires an ADR and migration strategy.

---

# 2. Required validation ladder

Every PR touching Rust code must pass, at minimum:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo build --workspace --all-targets
cargo deny check
```

For release candidates:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features --locked
cargo build --workspace --all-targets --locked
cargo doc --workspace --no-deps
cargo metadata --locked --format-version 1
cargo deny check
```

Additional commands become mandatory as the corresponding systems appear:

```bash
cargo test -p romueq-dsp
cargo test -p romueq-engine
cargo test -p romueq-analysis
cargo test -p romueq-integration-tests
cargo bench
```

Specialized tools such as model checking, property testing, allocation tracking,
sanitizers or host validation are milestone-specific and cannot be replaced by
the generic test suite.

---

# 3. Milestone map

| Milestone | Focus | Exit confidence |
|---|---|---:|
| M0 | Naming, scope and product contract | 95% |
| M1 | Repository, toolchain and CI foundation | 97% |
| M2 | P0 architecture prototypes | 99% |
| M3 | DSP mathematical foundation | 99% |
| M4 | Real-time engine | 99% |
| M5 | Analysis pipeline | 98% |
| M6 | GUI and interaction model | 98% |
| M7 | Plugin adapters | 99% |
| M8 | Standalone + CLI | 99% |
| M9 | Compatibility and hostile-host testing | 99% |
| M10 | Performance, reliability and release hardening | 99%+ |
| M11 | Release candidate and public distribution | 99%+ |

**Important:** the confidence numbers are gate targets, not automatic scores.
A failed P0 gate overrides every percentage.

---

# M0 — Naming, Scope and Product Contract

## M0.5 — Development System Hardening

M0.5 is mandatory before implementation proceeds beyond the product-contract stage. Its purpose is to convert this roadmap into a deterministic execution system for both humans and AI agents.

### M0.5 deliverables

- [`MASTER_PLAN.md`](MASTER_PLAN.md) — precedence, state machine, milestone control and confidence model.
- [`AI_DEVELOPMENT_PROTOCOL.md`](AI_DEVELOPMENT_PROTOCOL.md) — deterministic AI execution rules and stop-the-line behavior.
- [`TASK_SPECIFICATION.md`](TASK_SPECIFICATION.md) — mandatory task schema.
- [`tasks/TASKS.md`](tasks/TASKS.md) — executable task registry with dependencies, paths, tests and acceptance criteria.
- [`tasks/tasks.json`](tasks/tasks.json) — machine-readable task registry.
- [`MILESTONE_GATE.md`](MILESTONE_GATE.md) — gate procedure and automatic failure conditions.
- [`DECISION_REQUEST.md`](DECISION_REQUEST.md) — controlled mechanism for unresolved decisions.
- [`TEST_STRATEGY.md`](TEST_STRATEGY.md) — verification ladder and evidence requirements.
- [`COMPATIBILITY_MATRIX.md`](COMPATIBILITY_MATRIX.md) — host/platform validation contract.
- [`PERFORMANCE_BUDGET.md`](PERFORMANCE_BUDGET.md) — measured performance protocol.
- [`TRACEABILITY_MATRIX.md`](TRACEABILITY_MATRIX.md) — requirement-to-release traceability.
- [`RISK_REGISTER.md`](RISK_REGISTER.md) — P0/P1/P2 risk control.
- [`RELEASE_PROCESS.md`](RELEASE_PROCESS.md) — reproducible release procedure.
- [`EXECUTION_STATE.md`](EXECUTION_STATE.md) — live execution state.

### M0.5 exit criteria

1. Every implementation milestone M0–M11 has atomic tasks with dependencies.
2. Every task has explicit allowed/forbidden paths, tests, commands and binary acceptance criteria.
3. P0 uncertainties are represented as explicit Decision Requests.
4. A deterministic state machine defines when a task may start and finish.
5. Traceability exists from requirements to verification and release gates.
6. Compatibility, performance, security and release evidence are defined before implementation claims are made.
7. Static consistency validation passes with no orphan dependencies or missing task fields.

Run:

```bash
python3 scripts/validate_m05.py
```

M0.5 is `PASS` only when all seven criteria are satisfied.

## Naming gate — current status

**RomuEq is a working name, not a cleared trademark.** The exact string
`RomuEq` is already used by a municipal public-security organization in
Queimados, Brazil. M0 therefore cannot claim 99% naming confidence until
official trademark, domain and similarity searches are completed.



## Objective

Freeze what RomuEq is — and is not — before technical implementation.

## Tasks

- Complete final name clearance.
- Decide repository organization/name.
- Confirm licensing strategy.
- Confirm MVP feature list.
- Freeze supported operating systems.
- Freeze initial plugin formats.
- Freeze supported channel configurations.
- Define minimum host compatibility matrix.
- Define minimum CPU/performance targets.
- Define supported sample rates.
- Define supported block sizes.
- Define parameter ranges and defaults.
- Define state/preset compatibility policy.

## MVP contract

Initial target:

- 8-band parametric minimum-phase EQ;
- mono/stereo;
- f32;
- variable host block size;
- standalone;
- offline CLI;
- CLAP;
- VST3;
- meters;
- spectrum analysis;
- presets;
- versioned plugin state;
- sample-accurate automation where host APIs provide the required information.

Explicitly outside MVP:

- dynamic EQ;
- linear phase;
- oversampling;
- Mid/Side;
- multichannel;
- sidechain;
- WASM;
- AU/AUv3.

## Exit criteria

- product scope signed off;
- naming risk resolved or explicitly accepted;
- compatibility targets written;
- no ambiguous MVP feature remains;
- ADR-010 is consistent with the decision.

---

# M1 — Repository, Toolchain and CI Foundation

## Objective

Make the repository boring, reproducible and hard to break.

## Tasks

- Freeze Rust toolchain.
- Validate workspace dependency graph.
- Establish protected `main`.
- Require PR review.
- Enable GitHub Actions.
- Enable Dependabot.
- Enable dependency review.
- Enable `cargo-deny`.
- Add issue templates.
- Add CODEOWNERS.
- Add PR checklist.
- Establish documentation rules.
- Establish benchmark directory.
- Establish test fixture policy.

## CI matrix

Minimum:

- Linux x64;
- Windows x64;
- macOS x64;
- macOS ARM64.

CI must run:

- formatting;
- Clippy;
- unit/integration tests;
- workspace build;
- dependency/security checks.

## Exit criteria

A clean checkout must build and test without manual dependency edits.

**Evidence:**

- successful CI runs on every target;
- reproducible `Cargo.lock`;
- clean `cargo metadata`;
- no warnings treated as acceptable debt.

---

# M2 — P0 Architecture Prototypes

> **This is the most important milestone before production DSP.**

## Objective

Prove the architecture instead of trusting diagrams.

## M2.1 Parameter transport

Prototype:

- UI/control producer;
- host automation producer;
- bounded queue;
- RT consumer;
- overflow policy;
- ordering rules;
- sample offsets.

Test:

- empty queue;
- full queue;
- burst events;
- same-sample events;
- out-of-order producer attempts;
- variable block size;
- parameter changes at block boundaries.

Use model checking where appropriate.

### Gate

No unbounded queue.
No blocking operation.
No allocation in the RT consumer path.

---

## M2.2 Automation

Prove:

```text
host event
    ↓
block-relative sample offset
    ↓
engine event
    ↓
parameter application
    ↓
DSP state
```

Test:

- offset 0;
- offset 1;
- last sample;
- multiple events in one block;
- multiple events at the same sample;
- events spanning blocks;
- block-size changes;
- automation + preset/state interaction.

### Gate

The engine never silently converts sample-accurate events into block-rate changes.

---

## M2.3 Plugin framework

Prototype the smallest possible plugin:

- instantiate;
- process silence;
- process a sine;
- expose one parameter;
- automate one parameter;
- save state;
- restore state;
- open/close editor;
- suspend/resume;
- sample-rate change;
- block-size change.

Evaluate candidate frameworks against:

- CLAP;
- VST3;
- GUI embedding;
- state;
- automation;
- lifecycle;
- packaging;
- licensing;
- maintenance;
- cross-platform support.

### Gate

Do not freeze the framework until a real prototype works in representative hosts.

---

## M2.4 GUI embedding

Prototype:

- plugin editor creation;
- destruction;
- resize;
- HiDPI;
- keyboard focus;
- mouse input;
- multiple instances;
- host reopen;
- editor close/reopen.

### Gate

GUI framework is accepted only after embedding behavior is proven.

---

## M2.5 Lifecycle / reprepare

Prove:

- initial prepare;
- process;
- reset;
- sample-rate change;
- block-size change;
- channel-count change where supported;
- bypass transition;
- state restore before/after prepare.

### M2 exit criteria

All P0 decisions have:

- an ADR;
- a prototype;
- tests;
- documented evidence;
- a known failure mode;
- an explicit accepted behavior.

**No production DSP implementation starts before M2 is green.**

---

# M3 — DSP Mathematical Foundation

## Objective

Implement the EQ mathematics independently of plugins, GUI and host APIs.

## M3.1 Parameter model

Implement:

- stable parameter IDs;
- validation;
- normalization where needed;
- frequency limits;
- gain limits;
- Q limits;
- filter types;
- slope semantics;
- defaults.

Test every boundary.

Examples:

- minimum frequency;
- maximum frequency;
- minimum gain;
- maximum gain;
- minimum Q;
- maximum Q;
- NaN;
- positive infinity;
- negative infinity;
- invalid enum values.

Invalid floating-point input must have defined behavior.

---

## M3.2 Coefficients

Implement and validate:

- bell;
- low shelf;
- high shelf;
- high-pass;
- low-pass;
- required slopes.

For every filter:

- coefficient validity;
- finite values;
- stability;
- DC behavior;
- Nyquist behavior;
- expected gain;
- expected phase behavior.

## M3.3 Biquad processing

Test:

- impulse response;
- silence;
- constant input;
- sine waves;
- white noise;
- full-scale input;
- denormal-prone conditions;
- abrupt parameter changes.

## M3.4 Parameter smoothing

Define:

- smoothing domain;
- smoothing duration;
- behavior for frequency;
- behavior for gain;
- behavior for Q;
- behavior for filter-type changes.

Test:

- step response;
- rapid automation;
- repeated reversals;
- zero-length transitions;
- block boundaries.

## M3.5 Reference validation

Create analytical/reference tests for:

- magnitude response;
- stability;
- expected gain at center frequency;
- shelf behavior;
- cutoff behavior.

Where external reference implementations are used, document the source,
license and numerical comparison method.

## M3 exit criteria

- DSP passes analytical tests;
- reference audio tests pass;
- no NaN/Inf output under defined valid input conditions;
- numerical tolerances are documented;
- benchmark baseline exists.

---

# M4 — Real-Time Engine

## Objective

Turn the DSP into a production-safe processing engine.

## Tasks

- fixed-size band state;
- parameter snapshot;
- automation event application;
- coefficient update strategy;
- bypass;
- output gain;
- meter transport;
- reset;
- prepare;
- variable block processing.

## Real-time audit

Instrument and prove:

- zero heap allocation;
- no blocking mutex;
- no filesystem access;
- no logging;
- no GUI calls;
- no network;
- no unbounded data structure;
- bounded execution behavior.

## Bypass

Define and test:

- hard bypass semantics;
- click-free transition;
- state continuity;
- repeated bypass toggles;
- automation during bypass.

Prefer a deterministic crossfade if the measured behavior supports it.

## Engine tests

- mono;
- stereo;
- silence;
- impulse;
- sine;
- noise;
- full scale;
- arbitrary block sizes;
- block size 1;
- large blocks;
- parameter changes;
- automation bursts;
- reset;
- repeated prepare/reset cycles.

## Exit criteria

Real-time safety report exists and all critical checks pass.

---

# M5 — Analysis Pipeline

## Objective

Add spectrum/frequency-response analysis without contaminating the audio thread.

## Tasks

- FFT pipeline;
- spectrum calculation;
- frequency-axis mapping;
- smoothing/display policy;
- frequency-response calculation;
- history buffering;
- telemetry transport.

## Rules

The analysis worker may be expensive.

The audio thread may not wait for it.

## Tests

- known sine frequency;
- silence;
- white noise;
- impulse;
- changing sample rate;
- small and large analysis buffers;
- worker lag;
- dropped telemetry;
- shutdown/restart.

## Exit criteria

Analysis can be disabled or delayed without changing DSP correctness.

---

# M6 — GUI and Interaction Model

## Objective

Build a professional editor around the already-tested engine.

## UI areas

- EQ graph;
- band handles;
- band controls;
- frequency;
- gain;
- Q;
- filter type;
- slope;
- bypass;
- output gain;
- meters;
- spectrum;
- preset browser;
- undo/redo;
- A/B where applicable;
- settings/tools.

## Interaction tests

- mouse;
- keyboard;
- drag;
- double-click;
- modifier keys;
- keyboard focus;
- resize;
- HiDPI;
- multiple instances;
- automation feedback;
- preset load while editor is open.

## Critical rule

GUI state must not become the DSP source of truth.

The engine/parameter model is authoritative.

## Exit criteria

GUI is deterministic from parameter state and can be tested without a DAW
where practical.

---

# M7 — Plugin Adapters

## Objective

Connect the tested core to CLAP and VST3 without duplicating DSP behavior.

## Adapter responsibilities

- host lifecycle;
- audio buffers;
- parameter translation;
- automation translation;
- state save/restore;
- latency reporting;
- editor lifecycle;
- host notifications.

## Adapter rules

`romueq-dsp` must never depend on a plugin API.

`romueq-engine` must remain host-independent.

Plugin-specific code belongs in adapters.

## Host test matrix

At minimum, validate:

- plugin scan;
- instantiate;
- process;
- GUI;
- automation;
- save;
- reload;
- preset;
- bypass;
- sample-rate change;
- block-size change;
- multiple instances;
- offline render.

## Exit criteria

CLAP and VST3 are independently usable in the selected compatibility hosts.

---

# M8 — Standalone Application and CLI

## Standalone

Validate:

- device selection;
- sample-rate handling;
- buffer configuration;
- audio start/stop;
- device failure;
- settings persistence;
- preset loading;
- graceful shutdown.

## CLI / offline renderer

Support:

- input file;
- output file;
- sample-rate policy;
- channel policy;
- preset/state input;
- deterministic rendering;
- diagnostics.

## Offline reference tests

Render identical input twice.

Compare:

- duration;
- sample count;
- channel count;
- peak;
- RMS;
- numerical difference;
- expected tolerance.

---

# M9 — Compatibility and Hostile-Host Testing

## Objective

Test assumptions that unit tests cannot reproduce.

## Host matrix

Initial candidates:

- REAPER;
- Ableton Live;
- Bitwig;
- Studio One;
- Cubase;
- Ardour;
- Logic Pro where the selected future Apple format is supported.

## Host scenarios

For every supported plugin format:

1. scan;
2. instantiate;
3. process;
4. open editor;
5. close editor;
6. automate;
7. save project;
8. reopen project;
9. load preset;
10. change sample rate;
11. change block size;
12. bypass;
13. duplicate instance;
14. freeze/render;
15. offline render;
16. close/reopen host.

## Hostile scenarios

- zero-length processing if API permits;
- unusual block sizes;
- very small blocks;
- very large blocks;
- automation bursts;
- rapid editor open/close;
- rapid bypass;
- repeated project reload;
- state from older version;
- invalid/corrupted state;
- device/sample-rate changes.

## Exit criteria

No known host-specific blocker remains for the published compatibility matrix.

---

# M10 — Performance, Reliability and Hardening

## Performance budgets

Measure:

- CPU per stereo instance;
- worst-case CPU;
- memory footprint;
- GUI frame time;
- analysis worker CPU;
- startup time;
- plugin scan time.

Do not establish budgets from intuition.

Measure them on representative hardware.

## Stress tests

- thousands of parameter events;
- repeated prepare/reset;
- repeated state load/save;
- long offline renders;
- multiple plugin instances;
- rapid automation;
- sustained maximum signal;
- silence;
- denormal-prone content.

## Fuzz/property testing

Add property tests for:

- parameter validation;
- coefficient generation;
- state parsing;
- state migration;
- automation sequences;
- arbitrary block sizes.

Fuzz any parser accepting untrusted state data.

## Reliability gate

Run extended tests repeatedly and investigate every intermittent failure.

Flaky tests are defects, not harmless CI noise.

---

# M11 — Release Candidate

## Objective

Prove that the product can be shipped, installed, used and recovered.

## Release checklist

### Code

- [ ] No compiler warnings
- [ ] Clippy clean
- [ ] Formatting clean
- [ ] Tests green
- [ ] Integration tests green
- [ ] Property tests green
- [ ] RT-safety checks green
- [ ] Benchmarks within budget
- [ ] Dependency audit clean
- [ ] License policy clean

### DSP

- [ ] Reference responses validated
- [ ] Stability validated
- [ ] Automation validated
- [ ] Bypass validated
- [ ] Reset validated
- [ ] Sample-rate changes validated
- [ ] Block-size changes validated

### Plugins

- [ ] CLAP scan
- [ ] CLAP lifecycle
- [ ] CLAP automation
- [ ] CLAP state
- [ ] VST3 scan
- [ ] VST3 lifecycle
- [ ] VST3 automation
- [ ] VST3 state
- [ ] GUI embedding
- [ ] Multiple instances

### Distribution

- [ ] Version numbers synchronized
- [ ] Plugin IDs stable
- [ ] Artifacts named consistently
- [ ] Checksums generated
- [ ] Installation tested on clean machines
- [ ] Uninstallation tested
- [ ] Upgrade tested
- [ ] Release notes written
- [ ] Documentation complete

---

# 4. Rust Development Guidelines

## 4.1 General style

Prefer:

- small modules;
- explicit ownership;
- simple data structures;
- immutable data by default;
- exhaustive enums;
- newtypes for units/IDs;
- `Result` for recoverable errors;
- `Option` for absence;
- explicit conversions;
- documented invariants.

Avoid:

- giant modules;
- hidden global state;
- clever abstractions without measured value;
- trait objects in hot paths without evidence;
- unnecessary cloning;
- unnecessary allocations;
- implicit unit conversions.

---

## 4.2 Units must be explicit

Do not casually pass raw `f32` values where units matter.

Prefer types such as:

```rust
struct FrequencyHz(f32);
struct GainDb(f32);
struct QFactor(f32);
struct SampleRateHz(f32);
struct SampleOffset(u32);
```

This reduces accidental mixing of incompatible values.

---

## 4.3 Ownership and allocation

Before adding:

```rust
Vec::new()
String::from(...)
Box::new(...)
Arc::new(...)
```

inside code reachable from `process()`, stop and verify the real-time contract.

Prefer preallocation during:

```text
new()
prepare()
reset()
```

and reuse memory during processing.

---

## 4.4 No allocation in `process()`

Treat the audio callback as hostile to allocation.

Do not introduce:

- `Vec` growth;
- `String` formatting;
- iterator chains that hide allocation;
- collection construction;
- lazy initialization;
- logging;
- file access.

If an allocation is genuinely required, move it to a non-RT lifecycle stage.

---

## 4.5 Error handling

Never use:

```rust
unwrap()
expect()
panic!()
```

on an audio-thread execution path unless the invariant is formally proven
and the operation is unreachable under the validated contract.

Prefer explicit handling.

For configuration/state parsing:

```rust
Result<T, Error>
```

For impossible internal states:

- validate at construction;
- use types that encode invariants;
- fail before entering the RT path.

---

## 4.6 Floating-point rules

DSP code must explicitly define behavior for:

- NaN;
- positive infinity;
- negative infinity;
- subnormal values;
- overflow;
- underflow.

Do not silently assume all host inputs are well behaved.

Where numerical assumptions matter, add tests.

---

## 4.7 Traits and generics

Use traits when they clarify a stable abstraction.

Do not introduce a trait merely to make code "architectural".

In the DSP hot path, prefer predictable concrete data layouts unless profiling
and design evidence justify dynamic dispatch.

---

## 4.8 Iterators

Iterators are allowed.

The rule is not "avoid iterators".

The rule is:

> Use idiomatic Rust unless the generated behavior violates the real-time or
> performance contract.

Benchmark before replacing readable code with manual loops for performance reasons.

---

## 4.9 Comments

Comment **why**, not what.

Bad:

```rust
// Increment i
i += 1;
```

Good:

```rust
// Events are consumed in sample-offset order so automation remains
// sample-accurate when multiple events occur in the same host block.
```

---

## 4.10 Unsafe Rust

`unsafe` requires:

1. smallest possible scope;
2. explicit safety comment;
3. invariant documented;
4. test coverage;
5. review;
6. no propagation into safe core APIs unless unavoidable.

Unsafe plugin bindings must remain isolated from DSP.

---

# 5. Testing Guidelines

## 5.1 Test pyramid

### Level 1 — Unit

Fast, deterministic:

- parameter validation;
- coefficient equations;
- smoothing;
- state migrations;
- serialization.

### Level 2 — Property

Broad input space:

- random valid parameters;
- random block sizes;
- random automation;
- random state versions.

### Level 3 — Concurrency

Model and stress:

- queue;
- parameter transport;
- producer/consumer lifecycle.

### Level 4 — Audio reference

Known signals:

- silence;
- impulse;
- sine;
- white noise;
- full scale.

### Level 5 — Integration

- engine;
- analysis;
- GUI model;
- plugin lifecycle;
- state;
- automation.

### Level 6 — Host compatibility

Real DAWs and real plugin hosts.

---

# 6. Mandatory DSP test fixtures

Maintain canonical fixtures for:

```text
silence
impulse
20 Hz sine
100 Hz sine
1 kHz sine
10 kHz sine
20 kHz sine
white noise
full-scale sine
near-zero signal
```

Each fixture must have documented:

- sample rate;
- duration;
- channel count;
- amplitude;
- generation method.

Do not commit unexplained binary reference files.

---

# 7. Regression policy

Every discovered bug must produce, whenever possible:

1. a minimal reproduction;
2. a regression test;
3. a documented root cause;
4. a fix;
5. CI verification.

No "fixed manually" defects.

---

# 8. Git and PR discipline

## Branches

- `main` — protected;
- short-lived feature branches;
- PR required.

## PR size

Prefer one architectural invariant per PR.

Avoid combining:

- dependency upgrades;
- formatting;
- DSP changes;
- plugin changes;
- GUI redesign

unless necessary.

## PR must answer

- What changed?
- Why?
- What invariant changed?
- What tests were added?
- Does this touch the RT path?
- Does it change state/parameter compatibility?
- Does it require an ADR?
- What evidence supports the performance claim?

---

# 9. Definition of Done

A task is **not done** when code compiles.

It is done when:

- implementation exists;
- unit tests exist;
- relevant property/integration tests exist;
- documentation is updated;
- ADR is updated if needed;
- formatting passes;
- Clippy passes;
- tests pass;
- build passes;
- dependency checks pass;
- performance impact is measured where relevant;
- RT safety is reviewed where relevant;
- CI passes;
- reviewer approves.

---

# 10. Final 99% Gate

Before public release, conduct a formal release review.

## Architecture

- [ ] Every P0 decision has evidence
- [ ] No unresolved architectural ambiguity
- [ ] No accidental dependency from DSP to GUI/plugin host
- [ ] Thread ownership documented
- [ ] Lifecycle documented

## Real-time

- [ ] No allocation
- [ ] No blocking
- [ ] No I/O
- [ ] No hidden logging
- [ ] No unbounded queue
- [ ] No undocumented host calls
- [ ] Stress-tested

## DSP

- [ ] Mathematical reference tests
- [ ] Stability tests
- [ ] Numerical tolerance documented
- [ ] Automation tests
- [ ] Smoothing tests
- [ ] Bypass tests
- [ ] Sample-rate tests
- [ ] Variable block-size tests

## State

- [ ] Schema versioned
- [ ] Stable IDs
- [ ] Migration tests
- [ ] Corrupt-state handling
- [ ] Backward compatibility tested

## Plugins

- [ ] CLAP validated
- [ ] VST3 validated
- [ ] Host matrix validated
- [ ] GUI lifecycle validated
- [ ] Multiple instances validated
- [ ] Offline rendering validated

## Performance

- [ ] CPU budget measured
- [ ] Memory budget measured
- [ ] Worst-case behavior measured
- [ ] Representative hardware tested
- [ ] Long-run stability tested

## Security / supply chain

- [ ] `cargo deny check`
- [ ] dependency review
- [ ] license review
- [ ] dependency versions locked
- [ ] release artifacts checksummed

## Distribution

- [ ] Clean-machine installation
- [ ] Upgrade
- [ ] Uninstall
- [ ] Plugin rescanning
- [ ] Documentation
- [ ] Release notes
- [ ] Version consistency

### Release decision

Release only if:

```text
ALL P0 GATES = PASS
AND
ALL RELEASE BLOCKERS = 0
AND
CI = GREEN
AND
HOST MATRIX = GREEN
AND
RT AUDIT = GREEN
AND
DSP REFERENCE TESTS = GREEN
AND
PERFORMANCE BUDGET = GREEN
AND
STATE COMPATIBILITY = GREEN
```

If one of these conditions fails, the confidence target is **not met**.

---

# 11. Recommended implementation order

The implementation order is intentionally different from the order in which
the product is visually presented.

```text
M0  Scope / naming / contracts
 ↓
M1  Repository / CI / tooling
 ↓
M2  P0 prototypes
 ↓
M3  DSP mathematics
 ↓
M4  RT engine
 ↓
M5  Analysis
 ↓
M6  GUI
 ↓
M7  CLAP / VST3 adapters
 ↓
M8  Standalone / CLI
 ↓
M9  Host compatibility
 ↓
M10 Performance / hardening
 ↓
M11 Release candidate
```

The critical principle is:

> **Prove the foundations before multiplying them across GUI, plugins and
> platforms.**

---

# 12. First development sprint

The first sprint should contain **no production EQ implementation**.

### Sprint goals

1. Confirm project naming status.
2. Validate the Rust workspace.
3. Validate CI on all target platforms.
4. Write/approve ADR-001 through ADR-009.
5. Build the parameter-transport prototype.
6. Build the minimal plugin prototype.
7. Build the GUI embedding prototype.
8. Build lifecycle/reprepare tests.
9. Establish allocation/RT-safety test infrastructure.
10. Record benchmark methodology.
11. Review all P0 results.
12. Only then authorize M3.

### First sprint exit decision

```text
P0 architecture proven?
    |
    +-- NO --> prototype / ADR / test again
    |
    +-- YES
          |
          v
    authorize production DSP
```

This gate is intentionally hard.

---

# 13. Long-term engineering principle

RomuEq should optimize for:

**correctness → evidence → maintainability → performance → features**

not:

**features → complexity → debugging → architectural rewrite**

The architecture should make the correct implementation the easiest
implementation.
