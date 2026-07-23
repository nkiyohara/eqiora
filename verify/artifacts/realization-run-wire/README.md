# Realization and run wire verification

This case compiles the canonical Poisson model, resolves typed Realization
policy, serializes `eqiora.realization-envelope/v1`, and links it to
`eqiora.run-manifest/v2`. Both artifacts round-trip to identical canonical
bytes and domain-separated digests.

The `expected/` directory commits exact canonical JSON for Realization
envelopes v1, v2, and v3. Each version-specific decoder must accept its fixture
and re-encode the identical bytes. A serializer change therefore fails until
an explicit wire migration deliberately reviews and replaces the corresponding
golden fixture; comparing two outputs of the current serializer is not accepted
as compatibility evidence.

The falsifying paths pair an unrelated model with a resolution, omit required
distributed layout artifacts, change the executed reduction policy, and alter
the resolved worker count. Every mismatch must fail before a run manifest is
created or accepted against its referenced Realization.

Typed loopback/MPI/CUDA provenance vocabulary does not claim those adapters
are executable. Backend support remains gated by RFC 0010 evidence.

Run:

```bash
cargo test -p eqiora-artifact --test realization_run_wire
cargo run -p eqiora-verify -- run --case artifacts.realization-run-wire
```
