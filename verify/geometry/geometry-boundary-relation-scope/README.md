# Geometry-boundary Relation scope evidence

Precommitted evidence for the geometry-boundary Relation-scope contract, frozen by a non-implementing agent at base
`a5c122f` before any production code for the slice exists. It was frozen
**before registration**; see [Sequencing](#sequencing).

## Frozen claim

A continuous Relation may be scoped to a `GeometryBoundary` Domain when, and
only when, its `BoundaryOf` parent is an admitted `GeometryRegion` and its named
entity set is resolved against **that exact parent artifact** with dimension
`topological_dimension - 1`. For the exact circular-hole family only, the
single-edge `inlet` and `outlet` sets project their exact constant
parent-outward unit normals `[-1.0, 0.0]` and `[1.0, 0.0]`.

Boundary-physical Ports on geometry Domains keep their existing rejection.

## Non-claims

No geometry-boundary physical Port; no general boundary embedding; no circular,
multi-edge or per-member normal catalogue; no curved trace space; no embedded
manifold, 3D, mesh Realization, fluid lowering or solve; no new geometry kind or
public geometry representation.

**`walls` holds two edges.** This package therefore freezes only that `walls`
yields *no single normal*. It freezes neither a normal per wall member nor a
general index-to-normal catalogue.

One boundary is left explicitly **unfrozen**: a circular-hole set with exactly
one straight member under a name other than `inlet` or `outlet` (row
`accept-spare-unfrozen`). Both a name-keyed and a member-keyed implementation
satisfy this package, and neither is preferred here.

## The fixture is artificial

[`expected/boundary-scope-contract.json`](expected/boundary-scope-contract.json)
carries a small symbolic table, not copied implementation bytes. Handles `A`,
`B` and `C` are opaque stand-ins the implementing lane binds to real canonical
artifacts; no canonical byte string, byte length or content digest is reproduced
here. Members are side tags (`S-xlo`, `S-xhi`, `S-ylo`, `S-yhi`, `H-circle`,
`F-face`), so the oracle derives an outward normal from `(axis, side)` rather
than transcribing one.

Two rows exist to kill the two plausible wrong implementations:

- `accept-b-inlet` — artifact `B` names a **two-member** set `inlet`. A
  `match name { "inlet" => [-1,0], … }` lookup returns a normal here and fails.
- `accept-c-inlet` — artifact `C` is the straight-edged family with a
  single-member `inlet`. A family-agnostic single-side rule returns a normal
  here and fails.

## Mutations, detector and outcome

| Scenario | Input under test | Detector | Outcome |
| --- | --- | --- | --- |
| `accept-inlet` | single-edge `inlet` on its own parent | entity-set admission | accept, normal `[-1.0, 0.0]` |
| `accept-outlet` | single-edge `outlet` on its own parent | entity-set admission | accept, normal `[1.0, 0.0]` |
| `accept-walls` | two-edge `walls` | entity-set admission | accept, no single normal (`member-count`) |
| `accept-cylinder` | curved `cylinder` | entity-set admission | accept, no single normal (`curved-member`) |
| `accept-spare-unfrozen` | single straight side, other name | entity-set admission | accept, normal unfrozen |
| `accept-c-inlet` | straight-edged family | entity-set admission | accept, no single normal (`family`) |
| `accept-b-inlet` | two-member set named `inlet` | entity-set admission | accept, no single normal (`member-count`) |
| `accept-b-notch` | set taken from its defining artifact | entity-set admission | accept, no single normal |
| `reject-absent-set` | set absent from the parent artifact | entity-set admission | reject `EQ0302` |
| `reject-foreign-set` | same name present only in a sibling artifact | entity-set admission | reject `EQ0302` |
| `reject-wrong-dimension` | dimension-2 set on a boundary | entity-set admission | reject `EQ0302` |
| `reject-wrong-parent-kind` | `BoundaryOf` a Cartesian box | Domain topology | reject `EQ0302` |
| `reject-missing-artifact` | parent artifact absent from the bundle | closed-bundle index | reject `EQ0901` |
| `reject-port-on-boundary` | boundary-physical Port on the same boundary | support-use validation | reject `EQ0302`, unchanged text |
| `reject-free-relation` | artifact-free `from_snapshot` entry | support-use validation | reject `EQ0302`, unchanged text |

`reject-foreign-set` and `accept-b-notch` are one pair: the same set name is
rejected against `A` and accepted against `B` inside the same two-artifact
bundle. `reject-free-relation` is the sharp guard against simply deleting the
Relation arm of the existing support-use check.

This contract authors **no new diagnostic text**. Every sentence above already
exists at `a5c122f` and is frozen here as a compatibility obligation.

## Order invariance and compatibility

Every scenario is re-decided under every permutation of its supplied artifact
bundle; outcome, diagnostic and normal must be identical. Node identity ordering
is not re-derived here — RFC 0080 owns it.

The package also requires that the Model v7, structural-fingerprint and geometry
wire evidence listed under `compatibility_obligations` still exists and stays
green. It does not re-derive their values.

## Sequencing

At the freeze this directory had **no `case.toml`**, because the evidence is
frozen before the implementation exists and registration is integration-owned.
Two consequences, which pull in opposite directions:

- `eqiora-verify` collects manifests by walking for `case.toml`, so until that
  manifest exists it does not discover this package and no registered claim
  rests on it.
- `tools/ci/local_verify.py` derives case IDs from the **path**, not from a
  manifest. Any diff touching this directory therefore plans
  `--case geometry.geometry-boundary-relation-scope`, which
  `eqiora-verify run` rejects as an unknown case ID. **The fast and affected
  gates fail while this package stays unregistered.**

So registration is not merely deferred paperwork: the implementing lane must
apply this delta in the same change that first touches this tree.

1. Add `case.toml` with id `geometry.geometry-boundary-relation-scope`.
2. Amend `geometry-backed-semantic-admission/case.toml`, whose
   `[falsifiers] geometry_boundary_relation =
   "reject-non-Cartesian-embedding-contract"` and `[claim_boundary]
   boundary_support_consumer = false` become false once this slice lands, and
   whose README states that no accepted consumer observes boundary support.
3. Add the capability-matrix row.

Before step 2 happened, the repository held two contradictory registered
claims.

Step 1 is therefore something the oracle must survive, not forbid. It replays in
both states and reports which one it saw: `registration=frozen-before-registration`
while step 1 is outstanding, and `registration=registered` once this directory
holds a manifest whose top-level `id` parses as exactly
`geometry.geometry-boundary-relation-scope`. A manifest here declaring any other
ID, or one that does not parse, still fails, and so does any other manifest under
`verify/` claiming this ID. Nothing else about the manifest is checked: its
content is the implementing lane's to write, not this package's to own.

## Run

```bash
python3 verify/geometry/geometry-boundary-relation-scope/oracle/boundary_scope_oracle.py
```

## Not checked here

The corrected sequencing check reads the manifest's top-level TOML `id`, the form
every registered manifest in this tree uses. `eqiora-verify`'s own loader was not
read, so that agreement is a convention followed here, not one verified here.

No Rust was built, no gate tier was run, and no production behaviour was
observed — nothing implements this claim at `a5c122f`. The oracle checks the
frozen table against its own rule model, not against Eqiora. Whether the two
frozen normals are the ones later lowering actually needs is a claim of the
consuming slice, not of this package.
