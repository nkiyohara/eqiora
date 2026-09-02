# Transitive composed Model Package

This case closes one exact three-package component path:

```text
org.example.closed_circuit
  --circuits--> Eqiora.Electrical.Circuits
  --basic--> Eqiora.Electrical.Basic
```

The intermediate package exports one public `ParallelDc` component assembled
from the leaf package's `IdealVoltageSource`, two `Resistor` instances, and
`Ground`. The root declares only the intermediate package as a direct
dependency, instantiates that component, and binds its three typed scalar
parameters. The public component is deliberately closed: it has no physical
boundary Ports, so this case does not require or claim connection-set union
across a package boundary.

The registered target derives all three releases through the compiler, checks
the exact two-edge lock, installs each release through the ordinary atomic
single-release installer, reopens the store through its read-only capability,
and compiles the locked `Main` model. Reversing dependency input and in-memory
store insertion order must preserve root release, lock, Model, and compilation
bytes.

A source-only declaration and instance permutation of the intermediate
package must preserve its semantic identity, the root release, and the final
Model. Its source digest, resolution digest, and compilation identity must
change because those artifacts intentionally retain exact source lineage.
These are relational invariants within the authored variants; the case no
longer treats their incidental digest values as an oracle.

The flattened kernel is checked as a closed shape, including all nine current
node families, distinct identities for the two resistor instances, two
junction DAGs with root counts `[3, 4]`, and package-qualified provenance. The
root's three exact literals specialize through both Component levels, so the
flat kernel contains no fabricated Parameter node or alias. The provenance
chain is demonstrated on an ordinary resistor Port: its definition belongs to
Basic, its leaf instance belongs to Circuits, and its normalized binding spans
retain both the Circuits and root-package occurrence boundaries.

The resulting 14-by-14 affine system must recover 12 V, resistor currents of
6 A and 3 A, source current of -9 A, zero ground potential, and an accepted
original-DAG residual. Thus transitive reuse reaches execution through the
same flat semantic and numerical contracts; no package-specific solver path is
introduced.

Negative paths require an incomplete dependency closure, a transitive alias,
a private component, and a dimension-mismatched binding to fail with the
intended typed diagnostic. Before `PackageReleaseV1` exists, release
preparation compiler-validates every package Connector, Component, and Model,
including definitions that no selected Model instantiates. The mismatch in
this case therefore fails without synthesizing Model or Transaction graph
identity. A later `compile_locked` still replays the exact released semantics
and lowers occurrences from the selected Model; publication validation does
not replace that exact compilation boundary.

Run:

```bash
cargo test --locked -p eqiora --test composed_model_package
cargo run -p eqiora-verify -- run --case packages.composed-model-package
```

This evidence does not claim public physical boundary Ports, cross-boundary
connection-set union, imported Model references, package discovery, a
registry, network access, version ranges, signatures or trust, a
multi-package atomic installation transaction, dynamic plugins, nonlinear or
transient devices, general DAE consistency, runtime solvability, MPI, CUDA, or
broad component-library coverage.
