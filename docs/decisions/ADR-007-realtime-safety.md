# ADR-007: Real-time safety

Status: Proposed

The audio path must not allocate, block, perform I/O, perform syscalls, or intentionally panic.

The contract is enforced by tests and code review.
