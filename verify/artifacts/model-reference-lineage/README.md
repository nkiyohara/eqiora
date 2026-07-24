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

Every selected Model is decoded again through its public, explicit
`ExactModelCodec` selection before the Realization link is replayed. A second
test exercises the artifact owner's single historical-envelope dispatch
registration point by encoding and decoding the same semantic graph through
every v1-v6 generation. Every wrong-generation decode fails, and matching
Model identity and revision cannot replace the exact domain-separated artifact
digest.

This is identity-lineage evidence. The scalar-physical and field-boundary
Models are not lowered or executed, and no numerical or physical result is
accepted. Generation-neutral CAD and Geometry consumers do not retain
historical Model envelopes or dispatch on their generations. The artifact
registry does not own the public caller policy: adding a generation also
requires an explicit `ExactModelCodec` update. The contract performs no wire
detection, fallback, or schema upgrade.

Run:

```bash
cargo test -p eqiora --test model_artifact_reference_lineage
cargo run -p eqiora-verify -- run --case artifacts.model-reference-lineage
```

See [RFC 0037](../../../rfcs/0037-version-neutral-model-artifact-reference.md).
