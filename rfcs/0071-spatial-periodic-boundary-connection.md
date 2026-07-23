# RFC 0071: Spatial-periodic boundary connection

- Status: Implemented and verified for the bounded Cartesian FVM 2D slice
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

### Closed v1 identification profile

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
members and no boundary-family binder. Reusable periodic Component nets,
multi-generator lattices, and composition through public exposure cuts remain
outside this profile until their map-composition law is specified.

Endpoint order is presentation only. Canonical source identity orders the
pair structurally, while semantic lower/upper orientation is recovered from
the validated boundary geometry.

### Persisted compatibility

Model and Transaction wire v6 add only the new Connection value. V1 through
v5 reject it before encoding and retain byte-identical canonical fixtures and
domain-separated digests. V6 remains explicitly selected; decoders do not
sniff or retry older schemas.

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

## Research basis

- [MFEM periodic boundaries](https://mfem.org/howto/periodic-boundaries/)
  derive an explicit topological identification from translation maps.
- [OpenFOAM cyclic boundaries](https://doc.openfoam.com/2212/tools/processing/boundary-conditions/rtm/derived/coupled/cyclic/)
  require a paired topology and explicit transform; its conforming profile
  requires a one-to-one face map.

Eqiora adopts neither library's object model. The sources support separating
canonical identification, mesh correspondence, and numerical flux action.

## Nonclaims

This RFC does not claim rotations, general affine maps, phase/Bloch shifts,
vector or tensor fiber transforms, cross-parent pairing, nonmatching or
nonconforming faces, imported-mesh periodic metadata, periodic minmod,
periodic incompressible CFD, MPI, GPU, ALE, CAD quotient topology,
performance, or scale.
