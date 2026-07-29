# Model inputs

This interface case introduces no copied model fixture. The native composition
embeds the exact checked-in files from:

- `packages/Eqiora.Electrical.Basic`;
- `packages/Eqiora.Electromechanical.DcDrive`; and
- `packages/org.example.dc-motor-control`.

Their canonical author manifests, documentation entries, and model sources are
passed through `AuthorPackageSourcesV1` and
`prepare_package_release_v1`. The exact root and dependency releases then
derive `ResolutionRecordV1`; no discovery, alternate source spelling, or
application-owned model equation is admitted.

The registered `hybrid.packaged-dc-motor-controller` case owns the scientific
meaning and numerical reference for this exact package closure.
