# Compile/check control-plane verification

This case fixes one small application contract across Rust, Studio, and
Python. A closed `eqiora.control/v2` request selects the
`model.compile-check/v1` command and supplies only its request identity,
filename, and source. The authoritative Rust dispatcher then uses the ordinary
compiler, transaction commit, and current Model artifact path; callers cannot
select a Model wire or feature list.

The shared fixtures prove three distinct boundaries:

- accepted source produces only a typed current-Model identity descriptor,
  linked to the document from that same execution;
- syntactically rejected source produces a kernel diagnostic and no Model;
- retired or unsupported protocol and command identities are rejected by the
  bounded dispatch prelude before DTO admission or compilation;
- retired `modelWire` and `requiredFeatures` members are rejected by the
  closed v2 DTO; and
- request and diagnostic resource bounds fail closed without reflecting
  oversized caller content or publishing partial diagnostics.

Compiling the accepted source twice through control and once through
`ModelDocument::compile` intentionally produces distinct occurrence identity
and artifact digests. The independently registered structural fingerprint is
equal across all three compilations, while each accepted response is checked
only against the document returned by its own execution.

Client adapters may choose native function calls, Tauri invocation, or Python
objects, but they consume this meaning rather than recreating it. Responses
must not contain source, meshes, Fields, or trajectories. Preview, execution,
cancellation, artifact inspection/diff, remote transport, and bulk data remain
separate slices defined by
[RFC 0054](../../../rfcs/0054-curated-facade-and-control-plane.md).

Run:

```bash
cargo test --locked -p eqiora --test control_plane_compile_check
cargo run --locked -p eqiora-verify -- run --case interfaces.control-plane-compile-check
```
