# Typed package execution lineage

This case resolves and compiles the ordinary `org.example.poisson@0.1.0`
Model Package, then sends its canonical Model through the same typed scalar
elliptic Realization and Run v2 path used by package-free clients.

The package compilation, Model, Realization, and Run remain separate artifacts.
Only after a host-serial Q1 solve passes independently accepted true-residual
and continuous-balance checks does the application boundary construct
`PackageExecutionBindingV1`:

```text
exact package release + resolution
  -> PackageCompilationRecordV1 + current ModelEnvelope
  -> RealizationEnvelopeV1
  -> accepted host-serial Q1 execution
  -> RunManifestV2
  -> PackageExecutionBindingV1
```

The case round-trips the Realization, Run, and binding through their bounded
wire decoders and independently replays the complete chain. Reversing source
file insertion order changes no bytes or digest. A documentation-only package
change preserves package semantics and Model identity, but changes source,
resolution, compilation, and binding lineage. A different valid FVM
Realization, output identity, or execution producer cannot substitute for the
bound chain.

The binding is identity lineage, not execution evidence. Numerical acceptance
comes from the solve which gates construction; the edge itself neither attests
execution nor validates output contents. The Run is output-less because this
case does not invent a durable field-result artifact.

Run:

```bash
cargo test --locked -p eqiora --test typed_package_execution_lineage
cargo run -p eqiora-verify -- run --case packages.typed-execution-lineage
```

This evidence does not claim a package registry, ranges, signing, trust,
provider distribution, dynamic plugins, a general result artifact, imported
mesh, parallel execution, MPI, CUDA, adaptive realization, or general package
execution. The package selects no solver, mesh, target, or backend.
