# Packaged isotropic mechanical boundary in two dimensions

This case verifies the first public Model Package application of
[RFC 0041](../../../rfcs/0041-complete-exterior-port-families.md). The exact
`Eqiora.Solid.LinearElasticity@0.2.0` dependency adds a nominal
displacement/traction Connector and a separate `IsotropicMechanicalInterface2d`
Component without widening the already accepted volume-balance Component.

The root Model supplies one exact body, all four exact Cartesian exterior
Domains, and the occurrence displacement Field. Each generated boundary Port
has the package Connector and exact root Boundary identity. Each generated
Relation equates the Field trace to Port trace and parent-outward isotropic
traction to Port flux. Ordinary zero-traction terminal Components close the
four conserving sets with an explicit semantic natural condition, without
introducing a numerical boundary method.

The test changes exterior-member order and exact dependency-alias spelling
and requires identical package identity and canonical Model bytes. It also
pins the immutable package semantic and source digests, recursively matches
every flattened Relation expression, and counts the exact Port, Relation,
Activation, and Connection inventory. No family, set, package alias, or
numerical method survives into the Semantic Model.

Run:

```bash
cargo test --locked -p eqiora --test packaged_elastic_boundary_2d
cargo run --locked -p eqiora-verify -- run --case solid.packaged-elastic-boundary-2d
```

This evidence does not claim mesh facets, trace spaces, essential
elimination, a mixed-boundary solve, live coupled Port execution, Stokes, or
FSI. Those are separate Realization slices over this ordinary flat network.
