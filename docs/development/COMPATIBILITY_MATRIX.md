# Compatibility Matrix

## Status

The platform and format baseline is **FROZEN** by product-owner approval recorded 2026-09-03. Exact host/DAW versions remain a test-matrix decision and must be frozen before M7 compatibility validation.

## Frozen MVP platform/format baseline

| OS | Architecture | Plugin formats | Priority |
|---|---|---|---|
| macOS | ARM64 | CLAP, VST3 | P0 |
| macOS | x64 | CLAP, VST3 | P0 |
| Windows | x64 | CLAP, VST3 | P0 |
| Linux | x64 | CLAP, VST3 | P0 |

## Target hosts (version-neutral until M7 freeze)

| Host | Platform(s) | Scan | Instantiate | Audio | GUI | Automation | State | Offline | Multi-instance |
|---|---|---|---|---|---|---|---|---|---|
| REAPER | macOS/Win/Linux | Required | Required | Required | Required | Required | Required | Required | Required |
| Ableton Live | macOS/Win | Required | Required | Required | Required | Required | Required | Required | Required |
| Bitwig Studio | macOS/Win/Linux | Required | Required | Required | Required | Required | Required | Required | Required |
| Logic Pro | macOS | Required | Required | Required | Required | Required | Required | Required | Required |
| Studio One | macOS/Win | Required | Required | Required | Required | Required | Required | Required | Required |
| Cubase | macOS/Win | Required | Required | Required | Required | Required | Required | Required | Required |
| Ardour | macOS/Linux | Required | Required | Required | Required | Required | Required | Required | Required |

## Host scenarios

Every P0 host must be tested for:

- scan/discovery;
- instantiate/destroy;
- repeated start/stop processing;
- zero-length and small blocks where the host permits them;
- variable block sizes;
- sample-rate changes;
- automation at block boundaries and non-zero offsets;
- rapid parameter changes;
- state save/load;
- preset load;
- bypass;
- editor open/close/reopen;
- multiple instances;
- offline rendering;
- host suspend/resume.

## Version freeze requirement

Before M7 gate, record exact tested host versions, OS versions, plugin SDK/framework versions and test-machine identifiers. “Latest” is not an acceptable compatibility specification. This deferred host-version freeze does **not** block M0 platform/format closure.
