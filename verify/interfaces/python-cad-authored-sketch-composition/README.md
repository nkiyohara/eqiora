# Installed Python authored-sketch composition verification

This preimplementation case freezes one installed-Python projection of the
accepted opaque native `CadAuthoredSketch`. The new public type exists only as
`eqiora.geometry.CadAuthoredSketch`. It admits exactly a constrained XY
rectangle plus requested modeling tolerance or an exact circle retaining a
canonical v1 `end-cap` handle. The only graph-producing operations are
positive-z extrusion and one graph-bound circular through-cut. The existing
`CadAuthoredGraph.rectangle_extrusion` and `circular_through_cut` routes remain
supported with unchanged signatures.

The oracle introduces no CAD formula, expected value, tolerance, ordering, or
wire. Its rectangle fixture replays the accepted 731-byte v1 graph and digest
`919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36`.
Its symmetric cut fixture replays the accepted 1292-byte v2 graph and digest
`00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47`.
For each fixture, all applicable explicit and compatibility compositions agree
exactly, including canonical graph identity, face handles and order, every
public graph and face observation, the complete analytic receipt, each lineage
vector, and canonical decode/re-encode behavior.

The symmetric v2 graph is not asserted to project to the DFG planar witness. A
separate graph uses bounds `[0,2.2] × [0,0.41]`, plane `0`, modeling tolerance
`1e-10`, depth `1`, center `[0.2,0.2]`, radius `0.05`, and Boolean tolerance
`1e-10`. After all four graph routes compare equal, classification tolerance
`1e-12` and the accepted `fluid`, `inlet`, `outlet`, `walls`, `walls`, and
`cylinder` roles derive the accepted 511-byte Geometry with digest
`b00123472a596e8289820cabaee20d52cdf81b5572fa9ce58ff17cdaa00046d9`.
The separate installed-Python exact-geometry case remains authoritative for
the complete Geometry bytes and example output.

The explicit lineage vectors are compared directly with their compatibility
counterparts. Inherited membership/count assertions and a complete partition
of the public face inventory remain, but this case adds no new internal vector
order, complete handle-envelope, created-lineage, or provider-profile literal.
Rectangle lower-bound signed-zero equality is checked without applying a cut
to that boundary-zero rectangle. Circle-centre signed-zero equality is checked
separately on the symmetric authority.

Every value reaching native admission and violating the frozen contract must
raise `eqiora.ValidationError` with category `validation` and exactly one
nonempty kernel error diagnostic carrying `EQ0901`. The falsifier covers each
nonfinite rectangle and circle coordinate, reversed and degenerate bounds,
every nonpositive or nonfinite tolerance, depth, and radius, derived-end-plane
overflow, every wrong v1 face, a v2 face, foreign and stale graph bindings,
strict signed-clearance mutants, and every wrong operation order. Each
rejection also checks that observable graph, handle, and sketch inputs retain
their prior identity. Every branch compares the complete diagnostic message
with its accepted native owner: compatibility routes derive it dynamically,
while the wrong-face/v2, foreign/stale binding, rectangle-as-cut,
circle-as-extrusion, and second-cut branches pin messages derived directly
from the accepted native public API. Wrong wrapper types, tuple arity, and
nonnumeric Python values instead remain ordinary `TypeError` before native
admission.

Sketch equality is exact rather than variant-only or always true. Independent
equal rectangle and circle wrappers compare equal. Changed rectangle bounds,
plane, or modeling tolerance and changed circle center, radius, or retained
graph binding compare unequal; rectangle and circle variants also compare
unequal. These pairs make the atomicity snapshots capable of rejecting a
trivial always-equal implementation.

The lifetime oracle retains wrappers, constructs inline, drops source graph and
handle wrappers, and applies a retained sketch to a canonically replayed equal
target. Every route must retain exact v2 output identity. Runtime and installed
stub signatures, method inventory, sorted `__all__` parity, absence of generic
names and aliases, lack of a public sketch constructor, and absence of a root
`eqiora.CadAuthoredSketch` export are frozen. A second `python -I` subprocess
proves the explicit route from the non-editable installed wheel. The complete
runtime and stub export inventory is frozen as exactly `CadAuthoredBuild`,
`CadAuthoredFaceHandle`, `CadAuthoredGraph`, `CadAuthoredSketch`, and
`Geometry`, in that sorted order. Both sketch constructors must be actual
runtime and stub static methods.

Run after the implementation and integration lanes provide the frozen public
surface:

```console
cargo test -p eqiora-python --test python_cad_authored_sketch_composition
python3 tools/ci/python_package_gate.py
cargo run -p eqiora-verify -- run \
  --case interfaces.python-cad-authored-sketch-composition
```

At predecessor `5e92406a927b7e115fbfaa828bad76a30bbfa922`, the oracle is
expected to fail only because `eqiora.geometry.CadAuthoredSketch` and
`CadAuthoredGraph.through_cut` do not yet exist. The test files compile and the
case manifest validates before that expected missing-surface failure.

This case does not claim a general constraint solver; arbitrary planes,
profiles, curves, face-local frames, features, Booleans, or operation DAGs;
multiple, blind, or reordered cuts; sketch serialization or identity; mesh,
Model, solver, Result, Studio, visualization, performance, or scientific
validation; or mutation, undo, suppression, and history editing.
