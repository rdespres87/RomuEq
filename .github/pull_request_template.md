## Summary

<!-- What changed and why? -->

## Validation

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [ ] `cargo test --workspace --all-features`
- [ ] Allocation / real-time safety impact reviewed
- [ ] Documentation / ADR updated if architecture changed

## Real-time safety

If this PR touches `romuq-dsp` or `romuq-engine`:

- [ ] No allocation in the hot path
- [ ] No blocking lock
- [ ] No I/O / syscall
- [ ] No intentional panic path
- [ ] Threading contract reviewed

## Breaking changes

<!-- Parameter IDs, state format, public API, plugin behavior, etc. -->
