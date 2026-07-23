# Occurrence-bound component spatial supports

This case verifies that reusable Component definitions can name typed spatial
support slots without owning concrete Domains. Each occurrence binds those
slots to exact Domains in its enclosing scope, and ordinary hierarchy
elaboration rewrites the Component's Fields and Relations onto those existing
Domain identities.

The local fixture instantiates one Component twice over a shared two-dimensional
volume and two distinct exact boundaries. The two occurrences retain distinct
Field and Relation identities. Their volume and boundary slots do not become
Kernel entities or display aliases; the flattened graph instead contains exact
`DefinedOn` and `AppliesOn` edges to the enclosing volume and the corresponding
left or right boundary. Every elaborated member retains the complete occurrence
support-binding source provenance.

The exact-package path adds one forwarding Component between the root Model and
the Component that owns the Field and Relations. It therefore verifies two
levels of occurrence-bound forwarding through an exact offline dependency.
Compiling semantically equal roots with different dependency-alias spellings
must produce identical canonical Model bytes and digests. Package spelling is
lineage, not spatial meaning.

Negative cases require invalid support definitions and invalid occurrence
bindings to fail before a Model, Transaction, or graph mutation is exposed.
The matrix covers required and private slots, unknown and duplicate bindings,
volume/boundary kind and ambient-dimension mismatches, an inexact boundary
parent, and support-sensitive expression typing.

Run:

```bash
cargo test --locked -p eqiora --test component_spatial_supports
cargo run -p eqiora-verify -- run --case packages.component-spatial-supports
```

The claim is intentionally limited to scalar Fields and Relations over exact
Cartesian Volume/Boundary Domains in local and exact offline package
compilation. It does not claim field-valued Ports or the public interface work
defined by [RFC 0035](../../../rfcs/0035-field-valued-boundary-interfaces.md),
vector/tensor execution, mesh or Realization binding, a
numerical solve, fluid-structure interaction, or a reusable FSI package.
