# Current Model artifact canonical identity

This case owns the independent oracle for the one current Model artifact epoch
decided by [RFC 0083](../../../rfcs/0083-current-model-artifact-epoch.md). It
replaces the dangling `model.model-envelope-v7-canonical-identity` reference.

It answers one question: are the exact bytes and digests that the current Model
and Model Transaction owners must emit knowable *without* running the producer?
Every literal under `expected/` was derived by re-encoding the frozen public
fixture through the wire contract by hand and hashing it with Python `hashlib`.
The Rust test compares production output to those literals; it never derives
them. `references/derive_canonical_bytes.py` is the exact route.

## What is frozen

| Artifact | Bytes | Raw SHA-256 | Schema-domain digest |
| --- | --- | --- | --- |
| Model | 2347 | `7e179d0d…8aa427` | `e4102953…67e4b3` |
| Model Transaction | 2646 | `5ceeef06…8afc47` | `13216880…2aa102` |
| Cylinder Model resource | 16797 | `672016cb…65e099` | `8bc5155b…977146` |

The raw hash covers the canonical bytes exactly as stored. The schema-domain
digest is `SHA-256(schema ‖ 0x00 ‖ content)`, where Model content omits
`source_revision` because a graph revision is provenance rather than meaning,
and Transaction content is the complete envelope because operation order *is*
the artifact.

`expected/*.json` carry one trailing newline that the test strips; the frozen
byte counts are of the canonical payload without it.

## The historical corpus is negative only

`expected/historical/` holds fourteen literal specimens — Model and Model
Transaction for schemas v1 through v7. They exist to prove that the current
decoder refuses historical bytes outright and that relabelling one with the
current schema string does not make its meaning current. They are **not**
positive oracles: nothing here claims any historical generation remains
callable, and no historical decoder is used to read them.

Each specimen encodes one program containing a `cartesian-box` Domain, which
the current grammar refuses on meaning as well as on schema. v1 and v2 carry the
unshaped `field` representation and v3 through v7 carry `shaped-field`, so the
corpus is two content families rather than a schema relabel. It is deliberately
not a survey of each generation's full vocabulary — see `historical_vocabulary_survey`
in the claim boundary.

## The cylinder resource

`examples/steady-flow-past-cylinder.model.json` is the same semantic Model as
the superseded `…model-v7.json`, re-encoded into the current epoch. Its content
already lay inside the current vocabulary, so the re-encoding is exactly the
schema-identifier change — which is why the semantic Model ID
(`01KYQFNFX85DKM2SE5FR6H4WPJ`), the source revision (`1`), and the typed replay
are unchanged while the artifact identity moves from `668fa55e…` to `8bc5155b…`.

That is not a general migration rule. It works here only because this Model's
meaning is expressible in both grammars; the historical corpus shows the same
substitution failing when it is not. No digest is preserved across the change.

Replay stops at the typed reconstruction. This Model's Domains are
geometry-bound, so whole-model admission needs the geometry bundle and belongs
to the geometry-admitting layer. The registered cylinder cases consume this
current resource and own that claim; their fluid, geometry, mesh, balance, and
solver oracles remain independent of this byte oracle.

Run:

```bash
cargo test -p eqiora-artifact --test current_model_wire_oracle
cargo run -p eqiora-verify -- run --case artifacts.current-model-canonical-identity
```
