# Typed Model artifact reference lineage

This case projects explicitly selected Model v1, v2, and v3 artifacts to one
sealed typed identity contract, then feeds each reference to the unchanged
Realization v1 and Run v2 artifact chain:

```text
explicit Model v1 | v2 | v3
          -> typed artifact reference
          -> Realization v1
          -> Run v2
```

Every selected Model is decoded again through its explicit wire version before
the Realization link is replayed. A second test encodes the same semantic graph
in multiple Model wire domains and proves that matching Model identity and
revision cannot replace the exact domain-separated artifact digest.

This is identity-lineage evidence. The scalar-physical and field-boundary
Models are not lowered or executed, and no numerical or physical result is
accepted. The contract performs no wire detection or schema upgrade.

Run:

```bash
cargo test -p eqiora --test model_artifact_reference_lineage
cargo run -p eqiora-verify -- run --case artifacts.model-reference-lineage
```

See [RFC 0037](../../../rfcs/0037-version-neutral-model-artifact-reference.md).
