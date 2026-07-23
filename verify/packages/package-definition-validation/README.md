# Occurrence-free package definition validation

This case verifies the admission boundary introduced by RFC 0030. Before a
`PackageReleaseV1` exists, the compiler checks every Connector, Component, and
Model in the exact author source closure, including public, private, unused,
and non-selected declarations.

The positive fixture keeps a required public Parameter as a typed free
variable, uses it in a symbolic default, and forwards it through a nested
Component. Only the root Model supplies a concrete value. File input order and
declaration order preserve semantic package identity; changed author bytes
remain visible in the source digest.

Negative fixtures cover invalid unused Relations, missing and dimensionally
wrong nested bindings, a required private Parameter, an invalid second Model,
an unused invalid Connector, recursive and over-depth Component graphs, and
the spatial shape/support rules shared with whole-model semantics. A positive
occurrence-free definition proves that a shaped gradient Relation root means
componentwise zero; shaped Event and Guard roots remain separately rejected as
scalar activation conditions. The negative fixtures also
exercise the two independent obligations of scalar physical endpoints: one
Relation owner and one normalized conserving Connection membership. Public
Component Ports may export either obligation. A public child with an open
owner is admitted only after an explicit typed fragment fills its membership.
Repeated fragments, including an outer reconnection of an already connected
child boundary class, are idempotent topology claims. Private Ports and every
retained Model endpoint remain fail-closed. This case ends before occurrence
creation. The hierarchy compiler now has a separate provisional path that can
erase an ownerless exposure, but that path is not evidence supplied by this
case. Every failure here must occur in package preparation with an `EQ06xx`
source diagnostic and without a fabricated `GraphPath`.

Run:

```bash
cargo test --locked -p eqiora --test package_definition_validation
cargo run -p eqiora-verify -- run --case packages.package-definition-validation
```

This evidence does not prove equation squareness, solvability, stability,
convergence, execution success, behavioral validity for every future
Parameter value, field-valued physical boundary interfaces, occurrence-level
connection-set union or exposure elimination, imported Model references,
compatibility ranges, registry publication, signatures, trust, native
extensions, or dynamic plugins. Selected Model compilation remains the only
operation that creates occurrences, graph identity, and provenance.
