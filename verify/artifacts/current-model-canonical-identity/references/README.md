# Independent oracle route

`derive_canonical_bytes.py` is the complete route that produced every frozen
literal in `../expected/`. It reads nothing from the Rust producer at run time.

1. Read the frozen public fixture from
   `crates/eqiora-artifact/tests/current_model_wire_oracle.rs` — fixed ULID
   timestamp `1700000008000`, randomness `1..5`, the three coordinate axes, the
   Parameter value `2.0 m`, and the authored operation order.
2. Read the wire contract from the serde shapes in
   `crates/eqiora-artifact/src/model.rs`, `model/`, `model_wire.rs`,
   `model_transaction.rs`, and `model_transaction_wire.rs`: field names and
   declaration order, `kind`/`op`/`source`/`condition` tag names, kebab-case
   variants, `(kind, ULID)` node and edge ordering, and the fields the digest
   covers.
3. Re-encode the ULIDs (Crockford base32 of `ts << 80 | rand`), then the
   canonical JSON, by hand.
4. Hash with Python `hashlib`: raw `SHA-256(bytes)`, and the artifact digest
   `SHA-256(schema ‖ 0x00 ‖ content)`.

## What the route had to get right, and how it was checked

The Transaction reproduced `2646` bytes and digest `132168803a…` on the first
run, which confirmed the ULID encoding, every node encoding, the operation
order, and the digest construction independently of the Model.

The Model was `2153` bytes on that same run — exactly `194` short. The wire
contract explains the gap without consulting the producer:
`KernelNode::Parameter::initial_value()` returns `Some(value)`,
`Node::kernel` stores it, and snapshot admission copies it into `values`. One
`WireValue` for a length quantity is `194` bytes. Adding it reproduced `2347`
and `e410295337…`.

The cylinder route is cross-checked against evidence that already existed: the
same digest construction applied to the superseded v7 resource yields
`668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de`, which is the
superseded artifact identity retained by
`verify/artifacts/current-model-canonical-identity/case.toml` as a historical
cross-check, not an active decoder claim.

## Provenance of the negative corpus

The fourteen historical specimens were emitted once by the v1–v7 encoders that
still exist at base revision `975fe23`, so they are literal historical writer
output rather than hand-authored look-alikes. That generator was temporary and
is not committed: preserving the bytes is the point, and re-running a historical
encoder is exactly what RFC 0083 removes.

## Prior art

- [RFC 0083](../../../../rfcs/0083-current-model-artifact-epoch.md) — the epoch decision, frozen contract, and required falsifiers.
- [RFC 0078](../../../../rfcs/0078-direct-parameter-driven-cartesian-coordinates.md) — the coordinate-source meaning the fixture exercises.
- [RFC 0037](../../../../rfcs/0037-version-neutral-model-artifact-reference.md) — the superseded multi-generation compatibility policy.
