# Fixed-reference FSI solve on one CUDA device in 2D

This case owns the narrow composition between the exact fixed-reference FSI
operator and the existing generic CUDA execution contract. It does not create
a CUDA-specific FSI lowerer, assembler, physical finish, receipt, or artifact
schema.

The CPU-reference and CUDA-target Realizations independently finalize the same
canonical Model, authenticated mesh, previous state, quadrature, spaces,
scales, and one-step equations. Their complete CSR and right-hand side must
have the same exact agreement fingerprint. Placement changes only the solver
reduction policy: the CPU oracle uses reproducible identity-preconditioned
MINRES, while the CUDA path uses the adapter-native `Fast` policy with the
same symmetric-indefinite operator and identity preconditioner.

After a selected-device run, the replay reconstructs the exact Model and
`RealizationEnvelopeV3`, verifies `RunManifestV2` provenance and linkage, and
independently reapplies the finalized CSR on the serial host to accept the
recorded coefficients. Those coefficients then enter the sole pre-existing
FSI physical finish. An independently solved CPU oracle must agree under
`2e-10 + 2e-10 max(|a|, |b|)` for dimensionless algebraic coefficients and,
after exact Field identity/support/order checks, for velocity divided by `U`,
pressure by `P`, and displacement by `L`.

The general transfer, generation, fence, and graph-bound receipt machinery is
not reimplemented here. Its prerequisite is
[`numerics.canonical-cartesian-poisson-cuda`](../../numerics/canonical-cartesian-poisson-cuda/README.md).
This case records the unchanged generic receipt projection needed to show that
the FSI output used that path: exact operator/output identities, the nine-step
CUDA DAG, six transfers with no inverse-diagonal slot, three successfully
waited fences, resident-payload and external-workspace sizes, and the selected
device/runtime/library provenance. FSI meaning and physical acceptance remain
owned by
[`fsi.fixed-reference-monolithic-step-2d`](../fixed-reference-monolithic-step-2d/README.md).

The execution path and privacy-safe observation schema are implemented, but
this case is not registered as physical evidence in the initial public tree.
The previous private-development observation is deliberately excluded. After
the first clean public commit exists, the collector must run from that exact
clean commit, and the resulting source-linked observation must pass the
portable host replay before this case returns to `verified`. Until then there
is no public selected-device observation or hardware-compatibility claim.

After public recollection, the portable evidence command will be:

```text
cargo run -p eqiora-verify -- run --case fsi.fixed-reference-cuda-solve-2d
```

The case-specific collector is verification tooling, not a product result
format. It requires a release build, exactly one explicitly visible device, a
clean full-hex source commit, and a new output directory. Its versioned
observation records the public source commit and clean status plus bounded
selected-device and library information; it records no host identity or
private path.

```text
CUDA_VISIBLE_DEVICES=<physical-index> \
EQIORA_CUDA_DEVICE=0 \
cargo run --release -p eqiora --features cuda \
  --example fixed_reference_fsi_cuda_collect -- <new-output-directory>
```

No absence or failure may fall back to CPU. This bounded case does not claim
GPU assembly, matrix-free execution, reproducible device reductions,
multi-GPU or MPI plus CUDA execution, a trajectory, ALE, remeshing, durable
solution/receipt artifacts, hardware attestation, performance, or scale.
