# References

The oracle is the existing public Rust scalar-elliptic application service.
It owns typed FEM/FVM admission, exact-key Realization replay, assembly,
reference solve, independent true-residual verification, and continuous
balance acceptance. The PyO3 adapter contributes only frozen request/handle
ergonomics and bounded result projections.

Run provenance uses the existing `RealizationEnvelopeV1` and
`RunManifestV2` contracts. The execution receipt's exact L2 output fingerprint
is comparison evidence only; it is deliberately not promoted to a durable
Artifact identity.
