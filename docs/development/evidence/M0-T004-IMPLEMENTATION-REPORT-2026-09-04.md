# M0-T004 Implementation Report

**Status:** DONE

## Delivered

- Frozen licensing and dependency policy for MVP.
- Reviewed existing `deny.toml` allowlist and source restrictions.
- Explicitly prohibited wildcard dependencies, unknown registries, unapproved Git dependencies, VST2, and unapproved copyleft obligations.
- Defined dependency admission, exception, security, maintenance, and real-time suitability rules.

## External evidence

Current official Steinberg documentation states that VST3 is MIT-licensed since version 3.8 and distinguishes trademark usage from copyright licensing. The CLAP repository publishes an MIT license.

## Validation limitation

The execution environment used to prepare this repository does not provide a Rust/Cargo toolchain. Therefore this report does not claim successful `cargo deny check`, compilation, tests, or linting. Those remain mandatory CI/environment evidence.

## Acceptance

Policy deliverable is complete and internally consistent with `deny.toml`. M0-T004 is marked DONE; environment-dependent validation remains a gate requirement for the actual development environment.
