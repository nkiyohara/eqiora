# Expected values

[`boundary-scope-contract.json`](boundary-scope-contract.json) is the frozen
table. Its SHA-256 is pinned inside
[`../oracle/boundary_scope_oracle.py`](../oracle/boundary_scope_oracle.py), and
the sorted nonclaim slugs are pinned separately, so widening the table cannot
pass as running the oracle.

## Two independent routes

The table enumerates outcomes transcribed from the public claim in Issue #129.
The oracle re-derives every one of them from the fixture alone — outcome,
detector, diagnostic key, mutation classification and both normals — and the
run fails unless the two agree. The normals are derived from each member's
`(axis, side)` tag as a parent-outward unit vector, not transcribed.

## Column order

`scenario_columns` and `requirement_columns` are declared in the file. The
remaining tables are positional:

- `members`: `tag, kind, axis, side`; `axis` and `side` are null unless the
  member is a `straight-axis-side`.
- `artifacts.<handle>.sets`: `name, dimension, members`.
- `programs.<id>`: `artifacts_admitted, bundle, regions`, where each region is
  `region_id, artifact_handle, entity_set`.
- `diagnostics.<key>`: `code, text` and an optional third element `prefix`,
  which makes the text a required prefix rather than the whole sentence.
- `compatibility_obligations`: `obligation, evidence_path`.

## Provenance of the strings

Every diagnostic sentence in the file already exists in the accepted tree at
`a5c122f` and is frozen here as a compatibility obligation, not authored by
this package; `new_diagnostic_texts` is empty and the oracle enforces that. The
two parameterized sentences are additionally re-rendered from the fixture's own
dimensions, so a transcription slip in the frozen text fails the run.

No expected value for the new behaviour was derived from production output.
Nothing implements the claim at `a5c122f`, so there was nothing to read.
