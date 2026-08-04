# Independent transport derivation

`derive_provider_occurrence.py` uses only the Python standard library and the
accepted standalone prescribed-solid fixtures.  It verifies their canonical
bytes and lineage, extracts the two boundary traces, reconstructs the affine
candidate with exact binary64 arithmetic, builds every control and bulk frame,
and derives the transcript, occurrence, and two-output Run.

Run it without arguments to compare the derivation with all committed expected
files.  `--write` exists only to materialize this initial oracle precommit;
production writers must never use successor output to tune these fixtures.
