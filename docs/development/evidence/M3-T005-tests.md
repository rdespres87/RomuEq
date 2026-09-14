# M3-T005 — Test report

**Date:** 2026-09-14
**Milestone:** M3 — DSP mathematical foundation
**Task:** M3-T005 — Run M3 DSP gate

## Test summary

| Crate | Tests | Passed | Failed | Ignored |
|---|---|---|---|---|
| romueq-dsp | 43 | 43 | 0 | 0 |
| romueq-common | 130 | 130 | 0 | 0 |
| romueq-engine | 17 | 17 | 0 | 0 |
| romueq-integration-tests | 1 | 1 | 0 | 0 |
| Doc-tests (romueq-dsp) | 1 | 1 | 0 | 0 |
| **Total** | **192** | **192** | **0** | **0** |

## DSP test categories

### Coefficient tests (romueq-dsp)

- All filter types produce finite coefficients
- Bell filter: zero gain = unity, positive/negative gain, extreme Q
- High-pass: DC attenuation, passes high frequency
- Low-pass: passes DC, attenuates Nyquist
- High-shelf: zero/positive/negative gain
- Low-shelf: zero/positive/negative gain, DC behavior
- Frequency clamping to Nyquist and minimum
- Q clamping to valid range
- Sample rate assertions
- Normalization (unity gain at 0 dB)
- Default coefficients are unity

### Filter tests (romueq-dsp)

- Single channel unity passthrough
- Coefficient update preserves state
- Channel isolation
- Multi-channel interleaved processing
- Reset clears delay lines and history
- Reset is idempotent
- Panics on invalid input (zero channels, length mismatch, non-multiple, invalid channel)

### State tests (romueq-common)

- ParameterValues: new, set/get, remove, fill defaults, iter sorted, validate
- ProjectState: new, apply commands, serialize/deserialize roundtrip, lossless, is compatible, unknown product, unsupported future version, malformed input
- Preset: new, from state, serialize/deserialize roundtrip, unknown product, unsupported future version, malformed input
- Migration: rejects future version, noop at current version

### Automation tests (romueq-common + romueq-engine)

- Batch deduplication, ordering, deterministic
- Stream accumulation and finalization
- Value provider: linear interpolation, hold mode, no events returns last known
- Ingestor: default hold mode, drops out of range, reset, push/process block
- Processor: applies batch, overwrites values, store access, empty batch
- Full pipeline: deterministic ordering, malformed events, large block

## Reproducibility

All tests are deterministic. No external dependencies, no network calls, no random seeds. Results are reproducible across runs.
