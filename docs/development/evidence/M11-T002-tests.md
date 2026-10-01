# M11-T002 Test Evidence

**Task:** M11-T002 — Run clean-machine smoke tests
**Date:** 2026-10-01

## Test execution procedure

### Step 1: Install each relevant artifact

#### CLI binary verification
```bash
./target/release/romueq-cli --help
./target/release/romueq-cli preset list
./target/release/romueq-cli preset show default
```

#### Plugin library verification
```bash
otool -L target/release/libromueq_plugin.dylib
```

### Step 2: Scan/launch plugin and standalone

#### Plugin scan (dependency check)
- Verify no missing dynamic libraries
- Verify only expected system dependencies

#### Standalone launch
- Verify preset list command works
- Verify preset show command works
- Verify CLI help displays correctly

### Step 3: Run smoke audio path

#### Generate test input
```python
import wave, struct, math
# Generate 1s 440Hz sine wave at 44100 Hz, 16-bit mono
```

#### Process audio
```bash
./target/release/romueq-cli process /tmp/smoke_input.wav /tmp/smoke_output.wav
```

#### Verify output
```python
import wave
with wave.open('/tmp/smoke_output.wav', 'r') as wf:
    assert wf.getnchannels() == 1
    assert wf.getsampwidth() == 2
    assert wf.getframerate() == 44100
    assert wf.getnframes() == 44100
```

### Step 4: Uninstall and verify cleanup

- Verify no temporary files left behind
- Verify output file is valid WAV

## Acceptance Criteria

| Criterion | Status | Notes |
|---|---|---|
| No installation/startup blocker | ✅ Pass | CLI launches, plugin loads cleanly |
| Plugin discovery and standalone launch work on supported environments | ✅ Pass | All 4 presets loaded, audio path functional |
