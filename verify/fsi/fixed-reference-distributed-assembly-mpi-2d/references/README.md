# Reference provenance

The numerical and semantic fixture is reused directly from
[`fsi.fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/README.md).
No mesh, matrix, or expected-value table is copied into this case.

Each rank independently executes the ordered serial
`ReferenceAssemblyBackend` through the same canonical equation-aware discrete
block before invoking MPI assembly. The candidate cannot observe the reference
systems while deriving packet producers, row owners, route plans, or shards.
Only accepted reconstructed systems are compared bit-for-bit. The existing
serial-host MINRES and FSI finish path provide the post-assembly physical
acceptance oracle.
