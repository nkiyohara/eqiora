# Typed package compilation lineage

This case resolves and compiles the ordinary `org.example.poisson@0.1.0`
Model Package, then replays its canonical compilation record and current Model.
File insertion order does not change package or compilation identity. A
documentation-only source-bundle change preserves package semantics and Model
meaning while changing the source and compilation records.

The current fixture uses the compiler-owned `math.sin` spelling. The pre-1.0
namespace migration changed source and declaration identity, so the current
package, resolution, Model, and compilation identities moved together; the
lowered sine operation and scalar Poisson equation did not change.

The package still owns a self-contained `box(...)` Model and does not expose
abstract supports for caller Geometry. This case therefore makes no execution,
Plan, Run, numerical-acceptance, mesh, solver, or backend claim. Package
execution returns only when package authoring can enter the same caller-owned
Geometry and common root resolver lifecycle as ordinary `.eqi` components.

Run:

```bash
cargo test --locked -p eqiora --test typed_package_compilation_lineage
cargo run -p eqiora-verify -- run --case packages.typed-compilation-lineage
```
