# RomuEq Licensing & Dependency Policy

**Status:** FROZEN FOR MVP ENGINEERING
**Task:** M0-T004
**Date:** 2026-09-04

## 1. Policy goals

Dependencies must be legally distributable, auditable, reproducible, maintained, and compatible with the product licensing model. Dependency convenience is never sufficient justification.

## 2. Allowed license baseline

The current `cargo-deny` policy permits:

- MIT
- Apache-2.0
- BSD-2-Clause
- BSD-3-Clause
- ISC
- Unicode-3.0
- Zlib

Any dependency under another license is blocked until an explicit legal/product Decision Request is accepted and the `deny.toml` policy is updated intentionally.

## 3. VST3 and CLAP

The VST3 SDK is accepted for engineering use under its current MIT licensing model, subject to preserving required copyright/license notices. The VST trademark/logo is treated separately from copyright licensing; branding usage must follow Steinberg's current usage rules if used.

CLAP is accepted under its MIT license, subject to preserving the license notice.

No VST2 SDK/API dependency is permitted for MVP.

## 4. Dependency admission

Every non-trivial dependency must have a recorded review covering:

1. purpose and owning crate;
2. direct/transitive dependency impact;
3. license and attribution obligations;
4. maintenance/activity risk;
5. security/advisory status;
6. build-time/runtime impact;
7. real-time suitability if reachable from the audio path;
8. alternatives considered when the dependency is architectural.

## 5. Prohibited dependency patterns

- wildcard dependency versions;
- unknown registries;
- unapproved Git dependencies;
- dependencies that introduce GPL/AGPL/copyleft obligations into distributable MVP binaries without explicit legal approval;
- dependencies reachable from the audio hot path that allocate, block, perform I/O, log synchronously, or invoke uncontrolled callbacks;
- unmaintained P0 infrastructure without an explicit exception.

## 6. Supply-chain controls

The repository must use `Cargo.lock` for application/plugin/standalone/CLI reproducibility where applicable. CI must run `cargo deny check`; dependency review and Dependabot remain enabled. Advisory, source, and license failures are release blockers.

## 7. Exception process

An exception requires a `DR-YYYY-NNN` record before merge. The record must state the dependency, exact version/source, reason, license obligations, security impact, affected crates, alternatives, owner, expiry/review date, and gate impact.

## 8. Release rule

A dependency change is not release-ready until formatting, linting, tests, build, lockfile consistency, and `cargo deny check` pass in the real Rust environment.

## 9. Evidence rule

This document freezes policy; it does not claim that environment-dependent Cargo checks have been executed. Those checks remain mandatory CI evidence.
