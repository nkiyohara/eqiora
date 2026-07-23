# Reference provenance

The reference is the validated `ModelDocument` produced by an explicitly
selected `ExactModelCodec`. `ModelEnvelopeV1`, `ModelEnvelopeV2`, and
`ModelEnvelopeV3` independently derive the sealed identity projection from
their validated state. Realization and Run linkage is then checked against the
same selected artifact after explicit-wire canonical replay.

The executable authority is the `eqiora` integration test
`model_artifact_reference_lineage`; this directory adds no alternate identity
record or expected digest.
