# Private DFG cylinder mesh-family evidence

This pre-implementation package fixes one ordinary three-level primary family,
one same-fine-Geometry alternate-provider bias mesh, exact provider/probe/
spatial/time/topology identities, and all MESH0 structural falsifiers.

The positive path is source → fresh chordal Geometry at each level → bounded
Gmsh import → Mesh envelope → authored-region correspondence → conforming
binding → complete replay → ordered family → identity-only 3×3 space/time
association. Negative checks run only after that full positive succeeds.

Every imported fixture uses the structural Mesh admission gate
`minimum_mean_ratio = 1e-8` with exact binary64 bits
`0x3e45798ee2308c3a`; the Rust evidence asserts that the accepted Mesh retains
those bits. This is fixture-only admission input, not a scientific tolerance,
production quality target, or gallery-resolution choice.

The package proves private structural admission only. It does not prove DFG S1
or S2, a solve, accuracy, convergence, production resolution, force, pressure,
Strouhal, a pressure trace, a time method, or gallery readiness.

The Rust evidence maps the contract mutants directly:

- fixed polygon and each fake-refinement variant;
- foreign source/resource lineage, stale correspondence, and primary/bias swap;
- byte reencoding, node-index renumbering, and signed-zero-only fake bias;
- missing/overlapping source names, physical-group input, uncovered frontier,
  and solver-label attempts (the production input has no solver-facet-label
  field);
- recentered, reordered, off-circle, alternate-on-circle, wrong-normal, and
  nearest-node probe substitutions;
- every provider field drift, unknown role, metadata/receipt bounds, malformed
  or oversized MSH, non-triangle/nonfinite/inverted/degenerate/duplicate/
  low-quality topology, and declared resource excess;
- shortened/extended/ASCII-hex/mixed method carriers, bad time ordinals and
  steps, missing/duplicate/diagonal crossed cells, and S1 time leakage; and
- positive-first non-vacuity: every negative helper first admits and replays a
  fresh ordinary positive.

Import/correspondence failures are acceptable rejection locations for malformed
resources. Source names always come from the accepted authored-region
correspondence; the MSH and family inputs expose no replacement label channel.

Run the independent structural oracle with:

```bash
python3 verify/fluid/flow-past-cylinder-mesh-family-private/oracle.py
```

After production registration, the exact library-test selector is:

```bash
cargo test -p eqiora-numerics --lib canonical_stokes::navier_stokes_geometry_realization::mesh_family::tests:: -- --nocapture
```

The registered `case.toml` and capability-matrix entry are integrator-owned
and intentionally absent from this evidence branch.
