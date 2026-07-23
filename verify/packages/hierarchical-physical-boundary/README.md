# Hierarchical physical package boundary

This case verifies exact-package connection-set union through an ownerless
public scalar physical boundary. The dependency package exports
`ResistiveBranch`, whose public terminals forward to the owned terminals of a
private resistor occurrence. The root package closes those boundaries with a
source and ground.

Together with `language.hierarchical-connection-sets`, this case consumes the
`hierarchical-conserving-connection-sets-v1` conformance kit for deterministic
canonical-set observation. Exact-package identity, LCA ownership, provenance,
and the affine solution oracle remain case-owned assertions.

The two valid roots differ only in how they partition the negative net. One
uses a single N-ary conserving fragment; the other uses two overlapping
fragments. Source identity records that difference. Semantic package identity
and the canonical Model do not. Both forms must lower and solve to the same
physical result.

The same case seals both eliminated terminals into a versioned,
content-addressed projection catalog. Replaying that catalog requires the
exact package compilation as well as the flat Model because the latter alone
cannot reconstruct an occurrence cut. A separate post-run binding identifies
either the common across quantity or the net-outward through sum and one
existing Run output without embedding numerical values or introducing a Run
digest cycle.

Run:

```bash
cargo test --locked -p eqiora --test packaged_hierarchical_physical_boundary
cargo run -p eqiora-verify -- run --case packages.hierarchical-physical-boundary
```

The claim is intentionally bounded to exact offline packages, nominal scalar
physical Ports, static affine host execution, and one wrapper depth. It does
not claim field-valued interfaces, orientation, nonlinear or transient
physics, dynamic discovery, MPI, or GPU execution.
