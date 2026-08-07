# RFC 0071: Spatial-periodic boundary connection

- Status: Implemented and verified for the bounded Cartesian FVM 2D slice and
  one exact nonuniform `2 x 3 x 4` structural reference slice with unequal
  parent side lengths; the general arbitrary-count/axis-coordinate
  three-generator profile remains specified but is not implemented or verified
  as a class
- Authors: Eqiora contributors
- Created: 2026-07-22
- Depends on: [RFC 0035](0035-field-valued-boundary-interfaces.md) and
  [RFC 0069](0069-conservative-cell-centered-transport.md)

## Summary

Eqiora represents spatial periodicity as a typed relation between two exact
field-valued boundary Ports. It is neither a boundary-name convention nor a
mesh option. The first closed profile admits exactly two opposite Cartesian
boundaries of one parent Domain, one exact nominal Connector, a translation
derived from the parent bounds, and an identity map on the Connector value
space.

The bounded three-dimensional extension admits exactly three such pair
Connections for one Cartesian parent and one Connector, one pair per physical
axis. Their parent-derived translations and identity fiber maps form three
commuting, Connector-relative generators. A generated-Cartesian consumer may
derive a private cubical-torus entity and adjacency projection from that
accepted group; the unidentified box mesh remains the canonical mesh.

For the lower-to-upper translation `T`, the canonical junction means

```text
trace_upper(T(x)) = trace_lower(x)
flux_lower(x) + flux_upper(T(x)) = 0
```

where both fluxes use their parent-outward orientation. Model time remains
owned by `ClockKind::Periodic`; the new Connection semantics is explicitly
spatial and cannot stand in for a Clock.

## Decision

### Canonical owner

`ConnectionSemantics::SpatialPeriodic` owns the identification. Its two
`Connects` edges already name the exact Ports, and each Port already names its
exact boundary and nominal Connector. The translation is therefore uniquely
derived from validated Domain bounds; the Connection stores no duplicate
boundary IDs, floating-point translation vector, tolerance, mesh entity, or
face ordering.

The alternative of marking a Cartesian Domain axis periodic is rejected for
this slice. It would silently apply one topology to every Field on the Domain
and could not later distinguish identity, phase-shifted, or component-mapped
Field conditions. A quotient-Domain contract may be added independently when
a whole-Domain consumer requires it.

### One-pair identification profile

The first validator requires:

- exactly two `BoundaryPhysical` Ports;
- the same exact nominal Connector;
- the same exact Cartesian parent Domain;
- the same normal axis and tangential intervals;
- one lower and one upper side of that axis; and
- the Connector's existing shape, frame, dimensions, and Euclidean boundary
  duality, with no additional fiber transform.

The derived identification records the ambient dimension, normal axis,
lower/upper coordinates, period, and common tangential intervals. It is a
validated projection, not a second canonical payload.

Ordinary `Conserving` Connections retain coincident-support validation and
maximal-set normalization. Spatial-periodic pairs are not unioned
transitively with those sets. This avoids inventing translation composition,
phase accumulation, or cycle-consistency semantics.

### Source and hierarchy

The canonical source spelling is:

```text
connect periodic lower_port, upper_port;
```

The first authoring profile permits one exact closed-Model pair, including
resolved exact boundary selections. A periodic Connection has exactly two
members and no boundary-family binder. The three-generator Cartesian profile
below composes three independently valid pair Connections without changing
their cardinality. Reusable periodic Component nets, any other
multi-generator lattice, and composition through public exposure cuts remain
outside this RFC.

Endpoint order is presentation only. Canonical source identity orders the
pair structurally, while semantic lower/upper orientation is recovered from
the validated boundary geometry.

### Three-generator Cartesian 3D profile

#### Admission and ownership

For one selected tuple

```text
(parent P, nominal boundary Connector K, Connections C_0,C_1,C_2),
```

the three-generator profile requires all of the following:

- `P` is one exact Cartesian parent Domain of ambient dimension three with
  finite coherent-SI bounds `a_d < b_d` for `d in {0,1,2}`;
- every `C_d` is an existing `SpatialPeriodic` Connection with exactly two
  distinct boundary-physical Ports: the exact lower and upper boundaries of
  `P` normal to axis `d`;
- the three Connections and six Ports are distinct, and each Port belongs to
  exactly its one Connection;
- all six Ports name the same exact nominal Connector `K`;
- lower/upper geometry derives the normal-axis inventory `{0,1,2}` exactly
  once each, independently of declaration, Connection-identity, and endpoint
  order;
- every constituent pair passes the one-pair validator unchanged; and
- the fiber action in `K`'s existing component frame is identity. No phase,
  sign, rotation, reflection, permutation, shear, or other component map is
  inferred.

A repeated or missing axis, a fourth Connection for the selected `(P,K)`
family, a reused Port, a cross-parent group, a cross-Connector group, or a
partial axis inventory rejects this bounded profile. Unrelated periodic
Connections elsewhere in the Model do not join the group by proximity.

The group is Connector-relative rather than Domain-global. It does not make
another Connector or Field on `P` periodic. A physical-Field consumer must
separately validate that the complete Relations on all six Ports bind the
same exact Field trace and flux or traction meaning.

The existing semantic validator remains the sole owner of each pair. A
group-level validator beside it owns common `(P,K)` identity, exact axis
coverage, structured commutation, and identity-fiber cycles. A
generated-Cartesian correspondence owner may derive entity orbits and
incidence only from that accepted semantic group and one exact mesh revision;
a numerical consumer may not reconstruct canonical periodic meaning from
axis names or modulo indices alone.

#### Structured translation group

Define the exact positive periods and positive-axis generators by

```text
ell_d = b_d - a_d > 0,
T_d(x)_r = x_r + (if r=d then ell_d else 0).
```

The canonical generator identity is

```text
(exact parent P, axis d, lower bound a_d, upper bound b_d, identity fiber K).
```

No floating translation vector, matching tolerance, transform registry, or
endpoint-selected sign is canonical payload. `T_d` maps lower to upper and
`T_d^-1` maps upper to lower.

For `m=(m_0,m_1,m_2) in Z^3`, the lifted action is defined structurally:

```text
T_m(x)_d = x_d + m_d ell_d.
```

Thus, for distinct axes and every permutation `pi`,

```text
T_d T_e = T_e T_d,
T_d T_e T_d^-1 T_e^-1 = identity,
T_0 T_1 T_2 = T_pi(0) T_pi(1) T_pi(2).
```

Validation compares the three integer coefficients and their parent/axis
identities, not order-sensitive repeated floating addition. This exactness is
structural; it does not claim bitwise associativity for arbitrary repeated
binary floating-point operations or representability of unbounded lifts.

For each axis, the one-pair laws remain

```text
trace_upper_d(T_d(x)) = trace_lower_d(x),
flux_lower_d(x) + flux_upper_d(T_d(x)) = 0.
```

Trace values and component ordering use the identity fiber map. The flux sign
comes only from opposite parent-outward co-orientations. At a two-face
intersection both generator orders yield the same point, Connector value, and
component order; all six generator orders agree at a corner. Flux cancellation
remains pairwise by normal axis: this profile synthesizes no edge or corner
flux, traction, source, degree of freedom, or value.

#### Generated-Cartesian entity projection

Let the corresponding unidentified Cartesian box mesh have axis coordinates

```text
x_(d,0)=a_d < x_(d,1) < ... < x_(d,N_d)=b_d,
N_d >= 2.
```

All coordinates and every positive difference are finite and representable.
Write a box entity as `E(F,a)`, where `F` is its sorted set of free axes,
`k=|F|`, a free-axis anchor satisfies `0 <= a_d < N_d`, and a fixed-axis
anchor satisfies `0 <= a_d <= N_d`. For

```text
s_d(F) = N_d       if d in F,
         N_d + 1   otherwise,
rm(a;s) = (a_0 s_1 + a_1) s_2 + a_2,
```

the existing pre-quotient, last-physical-axis-fastest entity index is

```text
base_index_k(F,a)
  = sum_(F' lexicographically before F, |F'|=k) product_d s_d(F')
    + rm(a;s(F)).
```

The quotient anchor is

```text
q_d(E) = a_d mod N_d  if d not in F,
         a_d          if d in F.
```

Only a valid fixed upper anchor changes, from `N_d` to zero. Two box entities
are equivalent exactly when they have the same `F` and the same `q` in every
axis. The unique representative `E(F,q)` has all fixed anchors in
`0..N_d-1`, so lower-side copies own quotient identity. This is an identity
convention, not a flux-orientation convention.

For a canonical entity, let

```text
B(E) = {d : d not in F and q_d=0}.
```

Its complete box-representative orbit independently replaces `q_d=0` by
`N_d` on any subset of `B(E)` and therefore has size `2^|B(E)|`. A generator
toggles only its own fixed anchor, so generators on distinct axes commute.
Consequently:

- a seam vertex has orbit size two, four, or eight according to whether it
  lies on one, two, or three representative cuts;
- an edge may have orbit size one, two, or four from its fixed axes, and is
  never paired along its free axis;
- only lower/upper boundary faces normal to an axis have face orbit size two;
  interior coordinate-plane faces are singletons; and
- every cell is a singleton. Periodicity changes adjacency, not cell identity.

With

```text
C = N_0 N_1 N_2,
flat(q) = (q_0 N_1 + q_1) N_2 + q_2,
```

and `rank_k(F)` the zero-based lexicographic rank of `F` among size-`k`
free-axis sets, the derived quotient index is

```text
quotient_index_k(F,q) = rank_k(F) C + flat(q).
```

The quotient has exactly

```text
vertices = C,
edges    = 3C,
faces    = 3C,
cells    = C,
total entities = 8C,
Euler characteristic = C - 3C + 3C - C = 0.
```

These indices are a derived projection and do not replace or renumber the
current box `MeshEntity` inventory.

For local bit vector `b in {0,1}^F`, with bit zero attached to the first
sorted free axis, the quotient closure vertex is

```text
v_d = (q_d + b_d) mod N_d  if d in F,
      q_d                   otherwise.
```

The ordered closure is the existing tensor-product local vertex order.
Because `N_d>=2`, an entity's two endpoints on a free axis do not collapse.
Every generator preserves `F`, tangential anchors, and local bits, so the
exact correspondence is `VertexPermutation::identity(2^k)` and the current
`OrientationCode` is identity. A nonidentity permutation is not an equivalent
orientation choice. Parent-outward face normals remain separate: lower is
`-e_d` and upper is `+e_d`.

Every boundary-edge square has four box representatives and one quotient
edge when it lies on two cuts; both generator orders agree on entity, point,
vertex order, and fiber identity. The eight box corners form one quotient
vertex; all six generator orders and inverse-cycle insertions canonicalize to
that identity. Restricting a face correspondence to an edge or vertex agrees
with canonicalizing that lower-dimensional entity first: quotient and closure
operations commute.

The six codimension-one face interiors are covered exactly once by the three
pairs. Closures belonging to distinct axes intersect only in their expected
box edges or corners. Each paired face has the same tangential index, local
vertex order, measure, and translated point set. Exact axis and lower/upper
side remain recoverable for parent-outward trace and flux observations, and
quotient normalization retains which `C_d` authorized each seam.

#### Cell incidence and positive-axis packets

Cells use logical index `i` in
`Z/N_0 x Z/N_1 x Z/N_2` and cell index `flat(i)`. Define

```text
neighbor_plus(d,i)  = i + e_d mod N,
neighbor_minus(d,i) = i - e_d mod N.
```

Every quotient face has exactly two distinct incident cells, every cell has
six oriented face incidences, periodic adjacency is connected, and no
exterior face survives. The positive face of cell `i` on axis `d` has free
axes `{0,1,2}\{d}`, tangential anchors `i_t`, and fixed anchor
`(i_d+1) mod N_d`; its negative face has fixed anchor `i_d`. The quotient
face family rank for normal axis `d` is `2-d`.

A collocated consumer may derive one oriented positive packet per cell and
axis:

```text
packet(d,i) = d C + flat(i),
owner       = cell(i),
neighbor    = cell(neighbor_plus(d,i)),
normal      = +e_d.
```

This packet is a consumption projection, not another mesh identity, and maps
bijectively to the corresponding quotient positive face. At the seam
`i_d=N_d-1`, the owner is upper-adjacent and the neighbor is lower-adjacent,
lifted by `+ell_d e_d`. The upper parent-face view has normal `+e_d`; the
translated lower parent-face view has the opposite outward normal and signed
scatter. Lower-side ownership of quotient face identity and upper-adjacent
ownership of the positive packet are deliberately different roles.

There are exactly

```text
S_d = product_(r != d) N_r = C/N_d
```

seam packets on axis `d`. Each lower/upper facet pair names one packet. No
reverse duplicate is created, neither parent facet remains exterior, and
edges or corners create no packet work.

#### Geometry and degeneracy boundary

The admitted geometry is one conforming, full-dimensional, axis-aligned
Cartesian box. Mesh-axis endpoints equal the exact parent bounds and the
tangential axis arrays are shared across each pair. Axis counts and widths may
differ; uniformity, even counts, and a minimum count of four are not semantic
requirements of this profile.

Every `T_d` changes only coordinate `d` by the parent period and preserves
tangential coordinates, local vertex order, lengths, areas, and the component
frame. No geometric search participates. An axis with `N_d=1` is rejected:
it would collapse the endpoints of a free-axis entity and can make a normal
face self-adjacent. `N_d=2` is admissible because ordered closure and anchored
entity identities remain distinct.

Geometry crossing a cut is evaluated in a lifted chart. If the first and last
axis-`d` cell widths are

```text
h_first = x_(d,1)-a_d,
h_last  = b_d-x_(d,N_d-1),
```

the lower neighbor and facet are lifted by `+ell_d e_d`, giving centre
distance

```text
(h_last + h_first)/2 > 0.
```

Subtracting independently canonicalized half-open coordinates is forbidden.
This topology law specifies no stencil, transmissibility, interpolation, or
surface-rendering cut.

#### Resource boundary

Before allocating an orbit table, quotient topology, packet inventory, or
consumer state, checked integer arithmetic derives

```text
C = N_0 N_1 N_2,
box entities = (2N_0+1)(2N_1+1)(2N_2+1),
quotient entities = 8C,
quotient closure-vertex references = 27C,
orbits/mapping outputs = 8C,
box representative memberships across all orbits
  = (2N_0+1)(2N_1+1)(2N_2+1),
collocated positive packets = 3C,
seam packets = C/N_0 + C/N_1 + C/N_2.
```

Every product, sum, division, index conversion, and byte count is
overflow-checked. Raw axis-coordinate limits, current Cartesian
entity/closure caps, and the executor's existing entity, correspondence, and
byte limits must admit the complete abstract inventory before allocation. A
streaming implementation may store less but may not omit the count or
validation. Live allocation, hash-table load, traversal order, and worklist
lifetime are not resource oracles.

This profile creates no production ceiling or public resource knob. If an
existing resource owner cannot express fail-before-allocation admission for
the projection, implementation stops for a separately owned resource
decision; a consumer may not silently raise a mesh ceiling.

#### Persistence and public-API budget

The group, quotient, and packet views are private, non-persisted derived
projections unless a separately reviewed public-surface claim or two
independent external consumers justify otherwise. This amendment adds no
`PeriodicBox`, quotient-Domain, new `ConnectionSemantics` variant, public
transform registry or group-action trait, Model/Transaction/mesh/Result wire
field, mesh kind, persisted quotient entity, replacement `MeshEntity` ID,
crate, registry, or facade export.

If ordinary authoring, artifact replay, lowering, mesh correspondence, or
resource admission cannot carry three existing pair Connections without such
a public type, enum, Domain kind, wire field or version, schema, registry,
crate, or mesh identity, implementation stops and returns that exact
requirement to a separately budgeted public-contract decision.

#### Fail-closed boundaries

Implementation of this profile stops rather than widening the contract if:

- ordinary closed-Model authoring, current artifact replay, or semantic
  lowering cannot carry the six Ports and three existing pair Connections;
- common parent, Connector, exact axis coverage, or identity fiber cannot be
  recovered from current canonical meaning;
- edge or corner consistency requires a tolerance, order-selected transform,
  transitive conserving union, or consumer-owned interpretation;
- mesh pairing requires search, tangential permutation, a nonidentity local
  orientation, or persisted quotient authority;
- any cell is collapsed, a paired face remains exterior, or a seam has other
  than one positive packet;
- existing resource ownership cannot reject the complete checked inventory
  before allocation;
- implementation needs any new public or persisted surface excluded by the
  API budget; or
- an implementer would have to choose or tune an oracle value, tolerance,
  fixture, mutant outcome, or acceptance policy.

### Persisted compatibility

Model and Transaction wire v6 add only the new Connection value. V1 through
v5 reject it before encoding and retain byte-identical canonical fixtures and
domain-separated digests. V6 remains explicitly selected; decoders do not
sniff or retry older schemas.

The three-generator profile adds no wire vocabulary. A current Model persists
three ordinary existing Connection nodes and their existing edges. Its bytes
and digest differ because its Model content differs, not because a schema or
canonicalization rule changes. The current Cartesian mesh v1 envelope
continues to persist the conforming box axes and entities; the quotient is
rederived and revalidated rather than stored.

### Transport projection

The scalar transport Model keeps the existing volume Relation:

```text
derivative(c)
  + div(c * grad(psi))
  - div(kappa * grad(c)) = 0.
```

Each periodic side owns one boundary Relation binding the exact Port to both
the state trace and the total conservative flux:

```text
trace(c) - trace(port) = 0
normal(c * grad(psi) - kappa * grad(c)) - flux(port) = 0.
```

The lowerer recognizes that complete meaning and the validated periodic
Connection. It does not introduce a periodic PDE, infer periodicity from
names, or reinterpret a prescribed diffusive-flux law.

### Generated Cartesian FVM realization

Realization pairs lower and upper facets by exact tangential Cartesian index.
It validates a complete bijection, matching measure, and the parent-derived
cell-center distance before exposing assembly work. Each pair becomes one
oriented coupled face:

- its advective trace uses the same donor rule as an interior face;
- its diffusive action uses the same orthogonal two-point flux;
- its contribution scatters equal and opposite values exactly once; and
- it contributes no external boundary flux.

The initial profile admits only endpoint-evaluated first-order upwind across
the seam. Previous-state minmod needs periodic upstream/downstream provenance
and is rejected rather than silently degraded.

## Verification

### Implemented one-pair profile

The registered `fluid.cartesian-periodic-transport-fvm-2d` case must prove:

1. source formatting, endpoint permutation, and unrelated declaration order
   preserve canonical periodic meaning;
2. Model/Transaction v6 bounded round-trip and digest stability while all
   older wire goldens remain unchanged;
3. ordinary conserving rejection of noncoincident boundaries and periodic
   rejection of invalid cardinality, connector, parent, axis, side, or Port
   family;
4. a complete one-to-one generated-facet pairing with one packet per seam
   face and no seam contribution to external boundary flux;
5. a hand-calculated nonsymmetric action on a nonuniform probe vector that
   fails for an omitted, duplicated, mispaired, or orientation-reversed seam;
6. exact periodic cancellation, constant preservation, global conservation,
   and donor reversal;
7. a two-dimensional inflow/outflow problem that remains transverse-periodic
   and agrees with the independent one-dimensional spectral oracle; and
8. fail-closed unsupported reconstruction, forged pairing, non-finite value,
   and capability substitution.

### Three-generator profile obligations

The amendment itself is not implementation or evidence. The registered
[`fluid.cartesian-periodic-topology-3d`](../verify/fluid/cartesian-periodic-topology-3d/README.md)
exact reference case proves one ordinary positive path that:

1. authors six field-physical Ports on the six sides of one 3D Cartesian
   parent and three ordinary `connect periodic` declarations through one
   exact Connector;
2. formats and lowers declaration and endpoint permutations to the same
   axis-oriented group;
3. validates all three constituent pairs before the group law;
4. round-trips the complete current Model/Transaction artifact while
   preserving canonical identity under only already accepted permutations;
5. binds a non-cubic generated Cartesian mesh with unequal finite side
   lengths, counts `2 x 3 x 4`, and at least one nonuniform axis;
6. derives the complete quotient inventory, face pairings, edge squares,
   corner orbit, and positive packet view; and
7. independently replays those results from Model boundary identities and
   mesh axes rather than trusting a producer-owned orbit table.

For the `2 x 3 x 4` topology, this path must observe `C=24`, quotient strata
`(24,72,72,24)`, `192` quotient entities, `648` quotient closure-vertex
references, `72` positive packets, seam counts `(12,8,6)`, and singleton,
two-member, four-member, and eight-member orbits. These are structural
consequences of the contract, not implementation-produced expected data.

Only after that positive reaches the target gate may negative probes count.
An earlier parser, missing-Port, resource, or unrelated Model failure cannot
serve as rejection. The registered evidence must reject at least these
plausible wrong meanings at their named semantic, correspondence, incidence,
or resource boundary:

| Mutant | Required decisive observation |
| --- | --- |
| `P3D-ONE-PAIR-REUSED` | Fewer than three distinct Connections and axes fails the exact inventory. |
| `P3D-MISSING-AXIS` | A two-pair group fails exact `{0,1,2}` coverage after both pairs pass. |
| `P3D-DUPLICATE-AXIS` | Duplicate/missing axis keys reject otherwise valid pairs. |
| `P3D-CROSS-PARENT` | Common exact parent identity rejects composition. |
| `P3D-CROSS-CONNECTOR` | Common exact Connector identity rejects before fiber composition. |
| `P3D-ENDPOINT-ORDER` | Endpoint permutation retains the geometry-derived positive generator. |
| `P3D-STORED-VECTOR` | Replay from parent bounds rejects a supplied vector or tolerance. |
| `P3D-NONCOMMUTING-FIBER` | Two- and three-axis fiber-word replay rejects a nonidentity map. |
| `P3D-PAIRWISE-ONLY` | Missing edge/corner cycle receipts fails group admission. |
| `P3D-ORDINARY-UNION` | Typed, noncoincident generator identity rejects conserving-set normalization. |
| `P3D-FACE-ONLY-QUOTIENT` | Quotient counts, edge squares, corner orbit, and closure commutation fail. |
| `P3D-CORNER-ORDER` | Every two- and three-generator anchor/incidence word must agree. |
| `P3D-TANGENTIAL-SHIFT` | Exact anchors, points, local order, and nonsymmetric labels expose a shift or reflection. |
| `P3D-ORIENTATION-PERMUTE` | The required identity vertex permutation rejects reversal. |
| `P3D-OUTWARD-SAME-SIGN` | Parent-outward flux cancellation and packet scatter signs reject equal normals. |
| `P3D-CELL-COLLAPSE` | Singleton cell orbits, count `C`, and two-cell face incidence fail. |
| `P3D-SEAM-DOUBLE` | The `3C` packet bijection and `S_d` seam counts reject reverse duplicates. |
| `P3D-SEAM-EXTERIOR` | Zero exterior faces and two-cell incidence reject retained exterior work. |
| `P3D-LONG-SEAM` | Lifted distance must equal `(h_last+h_first)/2`. |
| `P3D-X-FASTEST` | Base, quotient, and packet replay reject noncanonical flattening. |
| `P3D-PERSISTED-QUOTIENT` | Artifact inventory rejects an unauthorized quotient field or version. |
| `P3D-ALLOCATE-FIRST` | Resource admission must precede allocation or mutation. |

## Research basis

- [MFEM periodic boundaries](https://mfem.org/howto/periodic-boundaries/)
  derive an explicit topological identification from translation maps.
- [OpenFOAM cyclic boundaries](https://doc.openfoam.com/2212/tools/processing/boundary-conditions/rtm/derived/coupled/cyclic/)
  require a paired topology and explicit transform; its conforming profile
  requires a one-to-one face map.

Eqiora adopts neither library's object model. The sources support separating
canonical identification, mesh correspondence, and numerical flux action.

## Nonclaims

This RFC does not claim rotations, reflections, shear, general affine maps,
phase/Bloch shifts, sign changes, component permutations, or other nonidentity
vector or tensor fiber transforms. It does not claim cross-parent or
cross-Connector groups; nonmatching, nonconforming, unstructured,
curvilinear, cut-cell, imported, CAD, ALE, remeshing, or moving topology; or a
quotient-Domain identity that applies to every Field.

The three-generator profile does not claim another dimension, a one- or
two-axis mixed periodic/open box, reusable periodic Component nets, public
exposure-cut composition, a public quotient mesh, persisted orbit data,
high-order trace or degree-of-freedom constraints, ghost or halo layout, or a
surface, clipping, welding, or rendering rule. Identity fiber allows the
Connector's existing component shape but does not establish numerical vector
or tensor periodic execution.

No collocated residual, pressure gradient, momentum-weighted coupling,
Newtonian traction, checkerboard action, time integrator, solver, JVP, energy
ledger, periodic minmod, periodic incompressible CFD, MPI, GPU, performance,
scale, or production-memory result follows from this semantic topology
contract.
