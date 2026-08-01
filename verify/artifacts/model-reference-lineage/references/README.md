# Reference provenance

The reference is the validated current `ModelDocument` produced by ordinary
source compilation or native definition. `ModelDocument` and the current
`AcceptedModelArtifact` independently expose the same sealed identity
projection from validated state. Realization and Run linkage is then checked
against that artifact after current canonical replay.

The executable authority is the `eqiora` integration test
`model_artifact_reference_lineage`; this directory adds no alternate identity
record or expected digest.
