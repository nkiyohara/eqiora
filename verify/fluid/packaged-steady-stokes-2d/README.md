# Exact-packaged steady incompressible Stokes law in two dimensions

This case closes one reusable, method-neutral fluid Component over the exact
offline package path. The immutable `Eqiora.Fluid.Incompressible@0.1.0`
release owns only one two-dimensional support slot, velocity, pressure, and
force-potential Field slots, one typed dynamic-viscosity Parameter slot, and
the momentum and incompressibility Relations. The root owns the unit square,
three Fields, the dynamic-viscosity Parameter, a nonconstant zero-mean
force-potential definition, and all four zero velocity traces.

The package and direct-flat fixtures must admit a complete deterministic
semantic-entity identity bijection and lower to the same canonical Stokes
problem. Expression-DAG sharing is not inferred across those two authoring
frontends; general algebraic/DAG normalization belongs to
[RFC 0073](../../../rfcs/0073-structural-semantic-fingerprint.md). Package
Model bytes are nevertheless exact under dependency alias, file order,
declaration order, and binding order. Renaming the provider preserves the
identity-normalized lowered action rather than package identity. Exact release,
resolution, compilation, and Model bytes replay offline.

Component Parameter arguments follow the typed-term contract in
[RFC 0055](../../../rfcs/0055-component-parameter-terms.md). Forwarding a root
Parameter retains that one differentiable identity. Binding a literal instead
specializes the occurrence to a typed constant: no child Parameter alias or AD
direction is invented, and changing the literal recompiles a new immutable
Model digest. Positive and negative zero have equal semantic package identity
and canonical Model bytes, while the exact source digest remains distinct.

Run:

```bash
cargo test --locked -p eqiora --test packaged_steady_stokes_2d
cargo run --locked -p eqiora-verify -- run --case fluid.packaged-steady-stokes-2d
```

This case makes no numerical execution, discretization, pressure-gauge,
natural-boundary, transient-flow, ALE, live-Port, or FSI claim.
