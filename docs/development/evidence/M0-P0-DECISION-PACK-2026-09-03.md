# M0 P0 Decision Pack — 2026-09-03

Purpose: provide decision-ready evidence without silently accepting any P0 Decision Request.

## DR-2026-001 — Product/name clearance

**Status:** OPEN — human/legal decision required.

### Current position
`RomuEq` remains a provisional display brand. This repository must not claim trademark clearance until a formal similarity search and, where appropriate, professional legal review have been completed.

### Required evidence
- EUIPO eSearch/TMview search covering exact and confusingly similar marks.
- Relevant national trademark databases for target launch jurisdictions.
- Domain and social-handle availability checks.
- Similarity review for audio software, plugins, music technology and adjacent Nice classes.
- Written decision identifying residual risk and acceptance owner.

### Recommendation
Keep `RomuEq` as the working display name during development, but use a release gate that prevents public release under that name until clearance is explicitly accepted.

## DR-2026-005 — P0 platform/host matrix

**Status:** OPEN — product-owner decision required.

### Proposed MVP compatibility baseline for decision
- Plugin formats: CLAP and VST3.
- Desktop targets: Windows 11 x86_64; macOS current supported Intel/x86_64 and Apple Silicon; Linux Ubuntu 24.04 LTS x86_64 for development/validation.
- Audio: mono/stereo, f32, variable host block sizes.
- Required host behaviors: scan, instantiate, activate/deactivate, processing, parameter automation, state save/restore, bypass, editor lifecycle and repeated instantiate/destroy.
- Host versions must be pinned before M1/M9 compatibility testing; the exact host/version list remains a product decision.

### Evidence note
The current Steinberg VST 3 SDK documentation lists Windows 11, macOS 10.14–26 and Ubuntu 24.04 LTS x86_64 among supported SDK environments, and the SDK includes a VST 3 Plug-in Test Host/Validator. This supports using those environments as an engineering baseline, but does not by itself define RomuEq's customer support policy.

### Recommendation
Adopt the above OS/architecture baseline for engineering, then explicitly name the customer-supported DAWs and exact versions in DR-2026-005 before M9.

## DR-2026-003 — Plugin framework

**Status:** OPEN.

### Evidence
The VST 3 SDK is MIT-licensed since version 3.8, including commercial binary distribution subject to the MIT terms. The official SDK also provides a Test Host/Validator.

### Recommendation
Prefer direct or narrowly isolated VST3/CLAP adapter layers over freezing the project around an unverified third-party abstraction. Any framework candidate must pass maintenance, license, ABI, sample-accurate automation, state, GUI, and hostile-host criteria before acceptance.

## DR-2026-002 — Parameter transport

**Status:** OPEN.

### Recommendation
Keep the previously specified bounded preallocated SPSC control-command queue plus a distinct host automation event stream preserving block-relative sample offsets. Require a prototype, stress test and explicit real-time evidence before acceptance.

## DR-2026-004 — GUI framework

**Status:** OPEN.

### Recommendation
Do not freeze a GUI framework until plugin embedding, standalone reuse, HiDPI/scaling, input latency, lifecycle/threading, packaging and accessibility constraints are tested on the selected platforms.

## DR-2026-006 — Performance reference

**Status:** OPEN.

### Recommendation
Define the reference machine(s), OS build, compiler/toolchain, sample rate, block sizes, band count, automation pattern, warm-up period and measurement protocol before converting the provisional CPU target into a release requirement.

## Decision rule
This document is evidence and recommendation only. It does not change any DR status and does not authorize implementation that depends on an unresolved P0 decision.
