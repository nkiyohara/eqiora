# Compile/check control-plane verification

This case fixes one small application contract across Rust, Studio, and
Python. A closed `eqiora.control/v1` request selects the exact
`model.compile-check/v1` command, required features, and immutable Model wire.
The authoritative Rust dispatcher then uses the ordinary compiler,
transaction commit, and Model artifact path.

The shared fixtures prove three distinct boundaries:

- accepted source produces only a typed Model identity descriptor;
- syntactically rejected source produces a kernel diagnostic and no Model;
- an unsupported protocol is rejected by the control boundary before its
  deliberately invalid source can reach the compiler.

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
