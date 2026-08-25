# Python common typed Plan verification

This case verifies the first common Python resolution spine without introducing
a second authority for the mathematics. The compiled `.eqi` `Model` determines
the supported scalar-elliptic operator. Python supplies a model-unbound typed
uniform Cartesian request containing only `cells_per_axis`, either the closed `fem.Q1()` or
`fvm.CellCenteredTpfa()` spatial policy, and the closed `solve.Linear()` policy.

One root `eqiora.resolve(...)` operation realizes the request against the
current Model Domain, then produces an immutable `Plan` that owns the exact
Model and effective Mesh and publishes their separate digests alongside the effective
space, quadrature, discretization, solver algorithm/backend, reduction,
placement, worker count, and Realization identity. `eqiora.run(plan)` executes
that stored binding without asking the caller to repeat the Model. Execution
reconstructs and authenticates the actual numerical Mesh against the exact Mesh
accepted during resolution.

Both policies are real consumers of the existing scalar-elliptic Q1 FEM and
orthogonal TPFA FVM implementations. Their scientific formulas, expected
values, tolerances, acceptance checks, and legacy `ScalarElliptic` /
`Realization` lifecycle are unchanged. The ordinary positives assert only
structural lineage and method-native Field location/count; existing numerical
cases remain the authority for scientific correctness.

The failure probes reject unsupported Mesh-request, spatial, and solve policy
types and a non-spatial temporal operator before Plan publication. A source
parameter edit can reuse the same geometric request. There is no universal option bag, string-keyed method registry,
generic polynomial order, inferred fallback, or promise that future spatial,
temporal, solver, backend, or placement policies compose.

This slice does not migrate geometry authority: the current `.eqi` Domain still
supplies the effective bounds. Later Python Geometry-to-Mesh work will remove
that remaining duplication boundary. The Plan publishes separate Model, Mesh,
and Realization identities, but no incomplete whole-Plan wire or digest.

Run:

```bash
cargo test --locked -p eqiora-python --test python_common_plan
cargo run --locked -p eqiora-verify -- run --case interfaces.python-common-plan
```
