# Evidence gallery experience contract

The evidence gallery is a publication surface over accepted Eqiora results. It
is not another result store, benchmark registry, or capability authority.
Registered manifests under [`verify/`](../../../verify/) own executable
evidence, the [benchmark roadmap](../benchmark-roadmap.md) owns case status,
and the [capability matrix](../../capability-matrix.md) indexes supported
claims. These experience specifications own only the public story and the
conditions under which that story may be published.

The eight retained flagship experiences are:

1. [laminar cylinder wake](cylinder-wake.md);
2. [thin cylindrical-shell collapse](shell-collapse.md);
3. [Turek--Hron FSI3](turek-hron-fsi3.md);
4. [Stokes minimum-drag shape optimization](stokes-shape-optimization.md);
5. [three-dimensional Taylor--Green breakdown](taylor-green-3d.md);
6. [notched-plate phase-field fracture](notched-plate-fracture.md);
7. [dam break around an obstacle](dam-break-obstacle.md);
8. [full electric-motor multiphysics](electric-motor.md).

This is the intended public delivery order, not a serial implementation lock.
Prerequisite verification lanes may run as soon as their contracts and
writable seams are disjoint. The order keeps the visible result sequence
dependency-closed: transient flow establishes the time-series path, nonlinear
structure follows, FSI consumes both, shape optimization reuses the accepted
flow, force, geometry-motion, and result-history seams, and the remaining
experiences add independent physics and scale.

## Publication admission

A production gallery build must fail closed unless every entry supplies all of
the following:

- an accepted Result or trajectory whose complete
  Model--Realization--Run--Result lineage resolves on the protected base;
- the exact registered evidence cases that support the public claim, without
  promoting their benchmark-roadmap status;
- content digests for every poster, video, text alternative, and downloadable
  result projection;
- the source-result digest, deterministic frame-selection rule, physical
  interval, presentation-time mapping, renderer identity, scene-profile
  identity, encoder identity, and producer environment;
- the exact public claim, important non-claims, quantity names and units, and
  the evidence-dossier route;
- a reduced-motion still and a text alternative that communicate the decisive
  observable without requiring the film.

Development previews may use synthetic or incomplete material only when they
are visibly marked non-publishable and cannot satisfy the production admission
predicate. A gallery entry never becomes evidence merely because its media
files have accepted digests.

The first implementation may keep this delivery record private to the site and
its build. A durable public artifact schema is introduced only when a second
real consumer proves the shared boundary. The normative requirements above do
not depend on that future encoding.

## Result and renderer boundary

The film consumes accepted result data; it never reconstructs a scientific
trajectory from storyboard keyframes. Any displayed vorticity, stress,
Q-criterion, force, energy, torque, or other scientific derived quantity must
already be an accepted, unit-bearing result projection owned by the scientific
case. The renderer may interpolate colors, place labels, choose a camera, and
construct explicitly presentation-only geometry such as deterministic
streamlines, but it may not turn those projections into evidence quantities.

Frame selection is a projection, not a new solve. It records source state
identities, never silently interpolates across topology changes, and states
whether it samples, decimates, or temporally interpolates. Fixed color and
deformation scales remain fixed across a film. Per-frame auto-ranging,
unlabelled clipping, and unlabelled deformation exaggeration are forbidden.

The authoritative result remains available independently of the film. A film
is a lossy presentation and is never a replacement for field data, comparison
tables, convergence studies, or the reproducible execution environment.

## Python composition admission

A proving workflow is admitted as complete only when it has one checked-in
ordinary Python script that imports the installed `eqiora` package and
consumes the same applicable Rust-owned Model, Geometry, Mesh, Realization,
Run, Result, or trajectory contracts as verification and Studio. It does not
recreate missing meaning in Python. A repository-only Rust executable, private
Studio bridge, or re-authored demo dataset may help development, but does not
complete the workflow. Native Rust tests remain scientific evidence; the
Python script is the reproducible user composition over that evidence.

## Film grammar

Each entry is a silent 12--18 second presentation:

1. roughly two seconds establish geometry, materials, and boundary conditions;
2. roughly eight to twelve seconds show one primary physical field at a time;
3. one or two decisive observables remain legible without pausing;
4. the closing frame returns cleanly to the poster composition.

The wall-clock duration is not physical time. Every film states its physical
time, load, or design-iteration interval and the presentation mapping. A
periodic phenomenon may close on a matched phase. An irreversible phenomenon
must not reverse its physics: it returns through a labelled neutral cut, fade,
or diagram reset.

Every field names the physical quantity and unit. Signed fields use a
zero-centred diverging scale when zero is physically meaningful; nonnegative
fields use a perceptually ordered scale. A camera change, mesh overlay, or
field transition occurs only when it serves the experience's single
responsibility. The detailed page, not the film, owns additional fields.

## Delivery and accessibility

Each accepted entry publishes:

- a lossless poster fallback and, when worthwhile, a modern compressed poster;
- one efficient modern video and one broadly supported fallback, each with an
  exact content digest and MIME type;
- intrinsic dimensions, duration, frame rate, and no layout-shifting load;
- responsive crops that do not remove the decisive observable or unit;
- a text alternative and a short ordered description of the visual changes.

Autoplay is disabled under `prefers-reduced-motion`. The still shown in its
place includes the geometry, primary quantity, physical state, decisive
observable, and evidence link. Color is never the only carrier of state.
Keyboard and screen-reader users can reach the same evidence dossier without
traversing video controls.

Encoded bytes are reproducible only within their recorded producer and encoder
profile. The contract promises exact identity of the published bytes and their
inputs, not cross-platform bit identity from an unspecified encoder.

## Evidence dossier

The dossier distinguishes four evidence kinds rather than collapsing them into
one badge:

- analytic or manufactured code verification;
- conservation, balance, and refinement evidence;
- accepted community numerical comparison;
- experimental validation within a stated validity range.

It records the exact problem and source, normalization and sign conventions,
lineage identities, mesh and time-step studies, decisive falsifiers, execution
environment, claim and non-claims, media provenance, and reference-data
licensing. Design guidance, such as a shell knockdown-factor recommendation,
is labelled as guidance rather than an exact oracle. A numerical community
benchmark, such as Turek--Hron FSI3, is not labelled experimental validation.

Derivation-bearing scientific slices use the repository's dual independent
oracle gate before implementation. Externally owned measurements or community
comparison values are not re-derived: their source, extraction procedure,
normalization, uncertainty, and acceptance band are pre-registered
independently of the implementer. The media adapter is an application surface
and needs focused contract tests, not a scientific oracle.

## Promotion

An experience moves through these independent states:

1. **contracted** -- this document fixes the public responsibility and
   non-claims;
2. **evidence-ready** -- every supporting scientific case is accepted at the
   status required by the claim;
3. **result-ready** -- an accepted content-addressed trajectory and all
   displayed result projections exist;
4. **publication-ready** -- media, dossier, accessibility assets, and the
   production admission check pass;
5. **published** -- the exact admitted entry is deployed.

No earlier state implies a later one. In particular, these future-state
experience specifications do not change any capability or benchmark status.
