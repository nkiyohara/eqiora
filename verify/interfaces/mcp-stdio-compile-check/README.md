# MCP stdio compile/check verification

This case proves one thin local subprocess projection of the accepted
transport-neutral `ModelDocument::compile` operation. A final MCP
`2026-07-28` client can discover the server, list exactly one in-memory
compile/check tool, and call it over newline-delimited stdio. The evidence
freezes framing, protocol and metadata admission, the exact tool definition,
error staging, bounded input and output, best-effort response cancellation,
and the one-active-call resource policy.

Request `_meta.progressToken` follows the final schema's `string | integer`
declaration. Integer means an exact integral mathematical value, so decimal and
exponent spellings with zero fractional value are admitted without relying on
lossy binary floating-point rounding. The raw witnesses distinguish that rule
from `is_number`, punctuation-only, and rounded-`f64` predicates. `1e400`
remains a decoder-level `-32700` framing failure rather than a metadata error.
A notification `_meta.progressToken` member is instead open, unrecognized, and
ignored like other valid notification metadata.

The accepted decay witness is compiled independently through the direct and
MCP paths. Their outcomes and generation-v3 structural fingerprints agree,
while their occurrence IDs and artifact digests differ. The MCP descriptor is
linked exactly to the document returned by that same MCP invocation without a
second compilation. Empty source is rejected by both paths with matching
normalized ordinary compiler diagnostics and no accepted Model.

The TextContent payload is compact JSON equal to `structuredContent`; the
result returns only a descriptor and comparison fingerprint. It returns no
source, canonical artifact bytes, artifact handle, persisted object, or
replayable Model.

The secrecy evidence is deliberately narrow: source, diagnostic-label
filename, cancellation reason, client Implementation and other non-reflected
metadata, compiler diagnostic text, and caught panic payload/location do not
enter protocol errors or stderr. This does not prohibit the accepted protocol
from echoing a safe response ID, reflecting the bounded requested version in
the unsupported-version error, or naming a well-formed bounded unknown tool.
Compiler diagnostic text and filename spans remain allowed only in the bounded
tool result.

This case does not claim Studio or Python integration, a CLI, HTTP or remote
transport, Apps, Tasks, resources, prompts, roots, sampling, elicitation,
logging or progress behavior, editing, solving, artifact inspection or diff, compiler or
solver cancellation, durable work, cross-request state, authentication,
deployment, generic MCP conformance, a reusable protocol layer, legacy MCP
compatibility, optional backend coverage, performance, fairness, or hostile
multi-tenant isolation.

Run:

```bash
cargo test --locked -p eqiora --bin eqiora-mcp
cargo test --locked -p eqiora --test mcp_stdio_compile_check
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.mcp-stdio-compile-check
```
