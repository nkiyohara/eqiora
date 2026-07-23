# Model inventory

`direct.eqi`, `mirrored.eqi`, `constant.eqi`, and `zero-advection.eqi` are complete, independently
compiled Semantic Models. The direct and mirrored models differ in canonical
potential gradient and in the vertical boundary Relations; no Realization flag
reverses the flow. The constant model differs only in its canonical
`concentration = 1 K` initial value and is the constant-preservation oracle.
The zero-advection model defines a constant potential and exact zero diffusive
flux on all sides; it proves that optional advective evidence does not narrow
the admitted canonical transport meaning.

The direct and mirrored Fields have the canonical scalar initial value `0 K`.
The executable initializer consumes that value directly; there is no second
callback or mesh-shaped initial-data channel. The positive-time spectral
solution in `problem.md` starts from the same zero field and is therefore an
oracle for the exact authored problem rather than a substituted Run input.

The `invalid/` inputs are well-typed source-level near misses whose meaning is
outside RFC 0069's closed lowering profile:

- `missing-inflow-relation.eqi` omits the trace closure required at the direct
  model's negative parent-outward velocity boundary.
- `outflow-trace.eqi` substitutes a trace law where positive outward velocity
  requires a diffusive-flux law.
- `mirrored-unswapped.eqi` reverses the potential gradient without exchanging
  the vertical laws, proving that filenames and boundary names cannot assign
  inflow or outflow roles.
- `nonaffine-potential.eqi` is a well-typed varying-velocity Relation outside
  the closed affine-potential profile.
- `varying-boundary.eqi` is a well-typed spatial boundary tape outside the
  closed constant boundary-data profile.

These files must compile as Semantic Models and fail during closed transport
lowering or pre-assembly boundary admission. Parser rejection would not prove
the intended numerical boundary contract.
