# Models

The executable sources are the ordinary package trees at
[`packages/Eqiora.Electrical.Basic`](../../../../packages/Eqiora.Electrical.Basic/),
[`packages/Eqiora.Electrical.Circuits`](../../../../packages/Eqiora.Electrical.Circuits/),
and
[`packages/org.example.closed_circuit`](../../../../packages/org.example.closed_circuit/).
The integration target admits their closed inventories directly, derives all
release and lock identities through the public facade, and installs the three
release wires into a temporary exact store.

`circuits-permuted.eqi` is an intentionally reordered form of the intermediate
package's model source. It is not a second library implementation. The test
substitutes these bytes into an otherwise equal closed source inventory to
prove that canonical semantic identity and the flattened Model ignore
declaration and instance ordering while source and compilation lineage retain
the different author bytes.

This directory is an evidence locator, not a package registry, discovery path,
or multi-package distribution format.
