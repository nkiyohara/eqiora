# Python native-modeling verification

This case verifies the bounded Python 0.1 model-construction surface without
creating Python-owned model semantics. Frozen Python handles wrap the
client-neutral Rust `ModelDraft`; `Model.define(...)` then uses the shared
compiler lowerer, selected transaction codec, atomic graph commit, and exact
Model artifact reconstruction.

The positive fixtures are one scalar decay Relation, one nominal
scalar-physical triple with a two-residual Relation and three-Port conserving
Connection, and one Cartesian constant-source Poisson model on a
one-dimensional interval with two oriented boundary Domains.
Python-produced artifacts replay through the public Rust facade. The decay
artifact executes through the Rust reference oracle, while the physical
artifact retains the exact Domain, Port, Relation, and Connection inventory.
The spatial artifact uses one continuum Representation, a supported scalar
Field, scoped continuous Relations, and only the closed `grad`, `div`, and
`trace` expression forms. It is reaccepted by the existing Rust scalar FEM
application path. All three Python fixtures compare equal to independently
compiled source through the shared generation-v2 structural semantic
fingerprint, while their exact Model digests remain distinct.

Falsifiers reject a same-named foreign Field, a dimensional mismatch, an
equal-looking but nominally foreign physical Domain, an omitted Connection
member, same-named foreign spatial Domain and Representation handles, a
Relation on a same-named foreign Domain, a boundary whose exact parent is
omitted, and a volume `trace` support mismatch. Draft
closure failures retain declaration paths; the support error comes from the
shared Semantic Kernel typing path. No failure exposes a partial Model.

This case does not compare exact artifacts from independent source and Python
authoring routes as equal. Those routes intentionally mint fresh occurrence
identities. Structural comparison delegates to the central Rust projection and
does not add a Python-specific normalizer.

Run:

```bash
cargo test -p eqiora-python --test python_native_modeling
cargo run -p eqiora-verify -- run --case interfaces.python-native-modeling
```

Shaped Fields, coordinates and other spatial functions, signal Ports,
clocks/events, components, hierarchy, declaration edits, Python Realization
control, callback operators, and complete native-language parity remain
outside this evidence.
