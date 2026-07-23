# Discrete field identity and import provenance

This case uses only the public `eqiora` facade to construct one affine-simplex
mesh, a Vertex scalar field, and an entity-major two-component Cell vector.
Two external-import observations retain different source bytes, names,
locators, and storage selectors while presenting equal normalized arrays.
Their accepted field bytes and digests remain identical, while their import
manifest bytes and digests differ.

Reference validation rejects an observation from the other manifest and a
reordered accepted-field list. Public construction also rejects non-finite
field data, field shape/count mismatches, and bounded field or manifest
decoding that exceeds caller policy.

`ExternalImportManifestV1` is deliberately an identity-checked lineage
assertion, not proof that source bytes derived normalized arrays or accepted
artifacts. This evidence therefore performs no format parsing and issues no
verified-lineage handle. XDMF/HDF5 and later VTU adapters must add their own
named deterministic replay evidence.

Run:

```bash
cargo test --locked -p eqiora --test discrete_field_import_provenance
cargo run --locked -p eqiora-verify -- run --case artifacts.discrete-field-import-provenance
```

See [RFC 0025](../../../rfcs/0025-discrete-field-and-import-provenance.md).
