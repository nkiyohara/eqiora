# Reference contract

The oracle is structural and independent of the implementation's allocation
order:

- [RFC 0040](../../../../rfcs/0040-occurrence-bound-field-slots.md) defines the
  exact Field type, occurrence binding, elimination, identity, provenance, and
  failure rules.
- [RFC 0034](../../../../rfcs/0034-occurrence-bound-spatial-supports.md) defines
  exact support substitution before Field admission.
- [RFC 0021](../../../../rfcs/0021-component-hierarchy-and-instantiation.md)
  defines deterministic occurrence identity and explicit-flat normalization.
- [RFC 0022](../../../../rfcs/0022-exact-package-identity-and-resolution.md)
  defines exact locked package resolution and dependency-alias normalization.

No floating-point baseline or second compiler supplies the expected result.
The test independently inspects the admitted Kernel graph, Relation expression
symbols, canonical envelopes, and pre-exposure diagnostics.

