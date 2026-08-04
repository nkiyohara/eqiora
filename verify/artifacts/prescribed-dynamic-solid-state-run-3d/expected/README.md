# Frozen exact artifacts

Each JSON file preserves canonical member and value bytes and has one trailing
repository newline. The nine lineage-bearing downstream fixtures place each
top-level member on its own line so the repository's frozen transition guard
observes exactly one Model-derived identity on the `model_` + `sha256` line.
Those insignificant newlines and the repository newline are removed by JSON
re-encoding before exact canonical-byte comparison. Every digest is derived by
hashing the schema identifier, one zero byte, and those compact canonical
bytes.

`model.json` is the exact current Model emitted by the accepted source at the
fixed predecessor. The standard-library derivation structurally validates it
and independently renders every downstream artifact. The four numerical
blocks use explicit binary64 producer bits. The prior and accepted velocity
sequences differ at those exact bits, so their canonical bytes and content
digests remain distinct.

These bytes are evidence, not a migration format, arbitrary artifact catalog,
or serialized acceptance report. They must not be regenerated from successor
production output.
