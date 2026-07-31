# Frozen bytes

`model-v8.json` and `model-transaction-v8.json` are the complete canonical
encodings of the frozen public fixture, derived independently of the Rust
producer. `historical/` holds the fourteen v1–v7 specimens that must be
**refused**, never accepted; see the case README before reading them as
anything else.

Each file carries one trailing newline. The frozen byte counts and both hashes
in `../case.toml` are of the canonical payload without it, which is what the
test compares after stripping.

Regenerating these from the current serializer would compare an implementation
against itself. Reproduce them with
`../references/derive_canonical_bytes.py` instead, and treat any disagreement as
a wire change requiring a deliberate epoch decision.
