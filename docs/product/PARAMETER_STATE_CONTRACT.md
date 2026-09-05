# RomuEq — Parameter & State Contract

**Status:** FROZEN FOR MVP
**Version:** 1.0
**Effective date:** 2026-09-04
**Owner:** Product + Architecture

This document freezes the logical parameter and persisted-state contract for the RomuEq MVP. Implementations may optimize the representation, but must preserve the externally observable semantics defined here.

## 1. Invariants

1. Parameter IDs are stable once released and must never be reused for a different semantic.
2. Parameter units and logical ranges are part of the contract.
3. Host-facing automation and internal DSP values are distinct representations.
4. Host automation events retain their block-relative sample offset.
5. GUI/control updates are delivered through the bounded control transport defined by ADR-001.
6. Persisted state is versioned and uses stable parameter IDs.
7. Loading an unknown future state version fails safely; it must not silently reinterpret fields.
8. Loading an older supported state version uses an explicit migration.
9. Presets and project/instance state are separate concepts even when they serialize the same parameter set.
10. No parameter may implicitly mutate another host-automatable parameter.

## 2. Stable parameter ID allocation

IDs are decimal `u32` values. The following allocation is frozen:

| ID range | Meaning |
|---:|---|
| 1–99 | Global parameters |
| 100–899 | Eight band parameter groups, 100–199 per band |
| 900–999 | Reserved for future MVP-compatible additions; no use in 1.0 |
| >=1000 | Reserved for future product-family features |

### Global IDs

| ID | Name | Type | Range | Default | Unit |
|---:|---|---|---|---|---|
| 1 | `global.bypass` | bool/stepped | 0..1 | 0 | — |
| 2 | `global.output_gain_db` | continuous | -24..24 | 0 | dB |
| 3 | `global.auto_gain` | bool/stepped | 0..1 | 0 | — |
| 4 | `global.processing_mode` | enum/stepped | 0..0 | 0 | — |
| 5 | `global.stereo_mode` | enum/stepped | 0..0 | 0 | — |

### Band IDs

For band index `b` in `0..8`, `base = 100 + b*100`:

| Offset | Name | Type | Range | Default | Unit |
|---:|---|---|---|---|---|
| +0 | `band[b].enabled` | bool/stepped | 0..1 | 1 | — |
| +1 | `band[b].filter_type` | enum/stepped | 0..4 | 0 | — |
| +2 | `band[b].frequency_hz` | continuous | 20..20000 | 1000 | Hz |
| +3 | `band[b].gain_db` | continuous | -24..24 | 0 | dB |
| +4 | `band[b].q` | continuous | 0.1..20 | 0.70710678 | Q |
| +5 | `band[b].slope` | enum/stepped | 0..3 | 1 | dB/oct |
| +6..+99 | reserved | — | — | — | — |

Thus band 0 occupies 100–199, band 1 200–299, ..., band 7 800–899.

## 3. Enum semantics

`filter_type`:

| Value | Meaning |
|---:|---|
| 0 | Bell |
| 1 | Low Shelf |
| 2 | High Shelf |
| 3 | High Pass |
| 4 | Low Pass |

`slope`:

| Value | Meaning |
|---:|---|
| 0 | 6 dB/oct |
| 1 | 12 dB/oct |
| 2 | 18 dB/oct |
| 3 | 24 dB/oct |

`processing_mode`:

| Value | Meaning |
|---:|---|
| 0 | Minimum Phase |

`stereo_mode`:

| Value | Meaning |
|---:|---|
| 0 | Stereo |

## 4. Host representation

The engine uses typed logical values. Adapters perform host-format conversion.

- **CLAP:** expose plain values/ranges as defined by the CLAP parameter contract; stepped/enumerated parameters use stepped semantics. CLAP requires stable parameter IDs and recommends persisting values through its state extension. citeturn0search0turn0search1
- **VST3:** expose normalized values to the host while preserving the same logical range and stable IDs internally. VST3 requires stable parameter identifiers and defines automation points by sample offset within the processing block. citeturn0search3

The adapter must never change the logical parameter meaning merely to fit a host representation.

## 5. Automation contract

An automation event contains:

- stable `ParamId`;
- logical target value;
- `sample_offset` in `[0, block_len)`;
- event ordering sequence for equal offsets.

For a block containing multiple events for one parameter, events are applied in ascending sample offset and then ascending sequence order. An event at offset `0` affects the block from its first sample. An event at offset `block_len-1` affects the final sample only, subject to the parameter smoothing policy defined by the engine.

The engine must not silently convert sample-accurate host events to a single block-level value. This matches the CLAP and VST3 automation models. citeturn0search0turn0search3

## 6. Persisted state schema

Logical state schema version: **1**.

Required envelope:

```text
schema_version: integer
product: "RomuEq"
parameter_values: map<stable_param_id, logical_value>
```

Optional non-parameter fields are permitted only under explicitly versioned namespaces. Unknown optional fields must be ignored. Unknown required schema versions must fail safely.

The canonical MVP state contains all global parameters and all eight band parameter groups, including disabled bands. This prevents omission-dependent defaults from changing future migrations.

State save/load is a main-thread operation at the plugin API level; audio processing must never perform persistence I/O. CLAP explicitly defines its state save/load callbacks on the main thread. citeturn0search1

## 7. Migration rules

- Version `1` is the first persisted schema and has no migration from an earlier RomuEq version.
- A future schema must provide a deterministic `N -> N+1` migration before it can be accepted.
- Migrations operate on logical values, not DSP coefficients.
- DSP coefficients and filter history are never persisted as authoritative state in MVP.
- A failed migration leaves the active instance unchanged.

## 8. Preset vs project state

**Project/instance state** contains the complete parameter state required to restore the instance.

**Preset** is a user-facing named snapshot of the same logical parameter schema. Preset metadata (name, author, tags, version) is not an audio parameter and must not become host automation.

## 9. Change control

Any change to a released parameter's ID, semantic, unit, range, enum meaning, or default requires:

1. a new ADR or explicit amendment;
2. migration/compatibility analysis;
3. regression tests;
4. host automation impact assessment;
5. gate approval before release.

No implementation may silently widen/narrow ranges or reuse an ID.
