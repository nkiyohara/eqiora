# Reference provenance

The numerical and semantic fixture is reused directly from
[`fsi.fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/README.md).
No mesh, matrix, or expected-value table is copied into this case.

The oracle is a separate execution of the existing ordered serial
`ReferenceAssemblyBackend` through the same equation-aware discrete-block
wrapper. The distributed candidate cannot observe that result while creating
cell packets, row owners, routes, or owner shards. Only after reconstruction
does the test compare both target systems bit-for-bit and compare the reduced
property-bound canonical fingerprint.
