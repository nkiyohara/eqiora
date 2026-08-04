# Installed Python projection of one exact Model Package

This case projects the already accepted `packages.offline-model-package`
fixture through the installed Python wheel. The caller supplies one explicit
content-addressed store root, the exact canonical resolution bytes, and the
bare root-local Model selector `Main`. The returned ordinary immutable
`Model` must retain the frozen Model identity while separately exposing the
exact package-compilation digest.

The registered executor runs the existing installed-wheel package gate and
the exact `eqiora-python` `python_offline_model_package` Cargo integration
target. The latter directly compares diagnostic fields and ordering with
`PackagedModelDocument::compile_locked`; neither oracle is implied by the
other.

The positive oracle reads the accepted store and deterministic Model and
compilation artifacts in place. Repository formatting adds one terminal LF to
the JSON fixtures; the public resolution input removes exactly that LF before
the call, and every other byte must equal the canonical wire. Expected Model
and compilation identities are never regenerated through the Python adapter.

The secondary `org.example.poisson` store fixture exists only to prove that a
valid `Model.commit(edit)` clears package lineage. Its release, resolution,
Model, and compilation identities were already accepted by
`packages.typed-execution-lineage`; this case adds no new Model or numerical
claim.

## Falsifiers

The installed-wheel and native tests reject non-canonical resolution bytes,
resolution/store mismatch, missing or hostile exact entries, non-bare Model
selectors, and invalid Python argument shapes. Every filesystem rejection is
checked against a before/after store snapshot. Source compilation, native
definition, replay, and committed children cannot acquire or retain package
lineage. Compiler diagnostics are compared field-for-field with the direct
Rust `PackagedModelDocument::compile_locked` result.

## Boundary

This is a synchronous, read-only Python projection of existing Rust package
semantics. It does not claim discovery, lock-file reading, workspace behavior,
package authoring or installation, network access, imported Model roots,
lineage persistence in Model JSON, execution, science, Studio behavior, or a
general package/provenance object.
