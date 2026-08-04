# Python control-plane verification

This case exercises the private PyO3 module as an in-process Python client and
then crosses back through the public Rust facade. Python keeps the exact
`compile(source, *, filename="<memory>")` call shape, performs its bounded
filename/source admission locally, releases the GIL, and invokes the existing
`ModelDocument::compile` operation exactly once without importing or
constructing a control DTO. A source-compiled current `Model` must replay
exactly through the one current owner: canonical bytes, artifact digest, typed
Model ID, and revision identity are preserved rather than recreated by Python.

Admission evidence fixes the 4,096-byte filename and 8,388,608-byte source
boundaries, including Unicode scalar-count limits, exact failure messages, and
the one-diagnostic `ValidationError` projection. Exact at-bound witnesses must
reach `ModelDocument::compile`; exact over-bound witnesses must fail before
compilation. Control diagnostic-count, diagnostic-member, encoded-response,
and overflow-substitution policies are deliberately not copied into Python.

Accepted frozen source compiled independently through Python, control-v2, and
direct Rust has pairwise-distinct Model occurrence IDs and artifact digests,
while all three generation-v2 structural fingerprints agree. Rejected source
has the same normalized ordinary diagnostics through all three paths. Identity
or digest equality is required only for a response and document from the same
invocation, never across independent compilations.

The same opaque `Model` previews one exact-base scalar value edit and commits
it atomically into a new immutable child. The base remains byte-for-byte
unchanged, the child advances the revision, and replaying the stale edit
against that child fails with structured diagnostics. Equal graph-local edits
prepared on divergent child artifacts retain distinct exact-plan identities.

Falsifiers also require malformed current wire, invalid source, invalid run
policy, and a deliberately panicking boundary operation to become the stable
compatibility, validation, execution, and internal Python exception families.
The guarded-thread panic hook must not emit the payload or Rust location, and
the Python diagnostic is `EQ0002`. Unrelated threads retain the process's
previous panic hook.
Initializing the native module must not import NumPy, PyTorch, or JAX, keeping
control-plane use independent of optional data and framework adapters.

Historical v1--v7 bytes and caller-selected codecs are outside the Python
surface; the canonical-identity case owns their negative corpus. This evidence
does not claim independent compilations have equal IDs or digests, nor does it
claim control-envelope overflow parity. Native
modeling vocabulary belongs to
`language.native-modeling` and its Python-specific follow-up. Async execution,
cancellation, progress, array exchange, DLPack, and framework integration are
also outside this case.

Run:

```bash
cargo test --locked -p eqiora-python --test python_control_plane
cargo run --locked -p eqiora-verify -- run --case interfaces.python-control-plane
```
