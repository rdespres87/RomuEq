# Release Process

## Release states

`DEV -> INTERNAL_RC -> HOST_RC -> RELEASE_CANDIDATE -> RELEASE -> POST_RELEASE`

No state may be skipped.

## Preconditions for Release Candidate

- M10 gate `PASS`;
- all P0 risks closed or explicitly accepted by authorized owner;
- no open P0 Decision Requests;
- all required host tests pass;
- state migration fixtures pass;
- performance budget passes;
- security/supply-chain checks pass;
- documentation and traceability are current.

## Artifact process

1. Create a clean checkout.
2. Use the pinned Rust toolchain.
3. Build with locked dependencies.
4. Run tests and package-specific validators.
5. Generate plugin artifacts for each target.
6. Generate standalone and CLI artifacts.
7. Record hashes, build metadata and tool versions.
8. Run clean-machine smoke tests.
9. Sign/notarize where applicable.
10. Store release evidence with the tag.

## Versioning

Product version, state schema version and preset schema version are distinct identifiers. A product version bump does not automatically imply a state-schema break.

## Rollback

Every release must have a documented rollback target and artifact hashes. A release may be withdrawn if a P0 defect is discovered.

## Post-release

Record host reports, crash reports, compatibility regressions, performance regressions and state/automation issues as risks/tasks. Never patch a release by changing a public contract without migration analysis.
