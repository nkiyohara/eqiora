# Evidence gallery experience contract

The gallery is a publication surface over real Eqiora Results: either clearly labelled
unverified product examples or accepted scientific results. It is not
another result store, benchmark registry, or capability authority.
Registered manifests under [`verify/`](../../../verify/) own executable
evidence, the [benchmark roadmap](../benchmark-roadmap.md) owns case status,
and the [capability matrix](../../capability-matrix.md) indexes supported
claims. These experience specifications own only the public story and the
conditions under which that story may be published.

Evidence development is active under [RFC 0089](../../../rfcs/0089-resume-claim-local-evidence-development.md).
New entries may ship first as clearly labelled product examples and later become accepted
scientific results when reproducible, independently derived evidence supports the exact claim.
Candidate oracles and falsifiers bind claim-local semantic projections; they do not pin unrelated
whole files, generated trees, or broad inventories.

The eight retained flagship experiences are:

1. [laminar cylinder wake](cylinder-wake.md);
2. [thin cylindrical-shell collapse](shell-collapse.md);
3. [Turek--Hron FSI3](turek-hron-fsi3.md);
4. [Stokes exact-area dissipation shape optimization](stokes-shape-optimization.md);
5. [three-dimensional Taylor--Green breakdown](taylor-green-3d.md);
6. [notched-plate phase-field fracture](notched-plate-fracture.md);
7. [dam break around an obstacle](dam-break-obstacle.md);
8. [full electric-motor multiphysics](electric-motor.md).

This is the intended public delivery order, not a serial implementation lock.
Product implementation lanes may run as soon as their contracts and
writable seams are disjoint. The order keeps the visible result sequence
dependency-closed: transient flow establishes the time-series path, nonlinear
structure follows, FSI consumes both, and shape optimization reuses accepted
exact geometry, fixed-topology harmonic motion, bounded steady-Stokes Result
and pressure, reduced-differentiation and immutable-history mechanics, and
common presentation seams. It consumes no force or drag. The remaining
experiences add independent physics and scale.

## Heavy result production

Start a heavy candidate only for a real gallery claim whose production-scale acceptance cannot be
established in ordinary PR conformance. Product-resolution output remains an unverified example
until that candidate is accepted.

Production-resolution solves, refinement campaigns, and complete media encodes
are governed by three separate authorities:

1. **PR conformance** uses small meshes, short trajectories, analytic or
   manufactured witnesses, admission mutants, and bounded renderer fixtures to
   exercise the ordinary implementation path cheaply. It does not accept or
   claim the production gallery result.
2. **Exact-head scientific candidates** run the full fixed scientific
   campaign explicitly for one final source revision in its declared trusted
   environment. Before the run, the claim, independent oracle, tolerances, and
   stop conditions are fixed; the candidate is accepted before the experience
   can become result-ready.
3. **Immutable publication projections** consume an accepted, immutable Result
   or trajectory. Publication and replay retrieve and verify the exact admitted
   media bytes; site deployment never solves the governing equations.

The current tiny fixed-reference FSI gallery case may retain its complete
450-frame development build because that bounded case is cheap. It is PR
conformance, not a production-resolution cost precedent or a full scientific
candidate.

Each heavy candidate binds the exact Model, Geometry, mesh family,
correspondence, Realization, Run, field and result identities, source revision,
producer and runtime environment, solver and library identities, independent
oracle and registered evidence IDs, output inventory, and content digests. The
first real consumer also fixes its private delivery record and durable external
delivery location. The repository retains the bounded case contract,
independently owned oracle or reference inputs, compact observations and
comparison summaries, receipts and manifests, digests, and small conformance
fixtures; bulk trajectories and media need not enter source history.

Invalidation is fail-closed and follows the changed meaning:

- an equation, lowering, assembly, boundary law, mesh family, time integrator,
  solver-acceptance rule, scientific observable, benchmark, or oracle change
  requires a new full candidate on the final affected head;
- a renderer, scene-profile, encoder, or accessibility change may project new
  media from the unchanged accepted trajectory; and
- documentation, site-shell, and unrelated product changes reuse the accepted
  result and admitted media unchanged.

A source change never transfers a successful candidate to another head by
assertion. If the affected heavy environment cannot run, the limitation stays
visible and the claim is narrowed or promotion stops. A small conformance pass,
stale bundle, or `not-selected` result cannot be relabelled as full scientific
acceptance. Each scientific slice names the exact registered cases and
invalidation inputs it affects; this contract does not define broad path-glob
automation.

An Actions cache, compiler cache, workstation cache, Vault copy, or site copy
may accelerate or mirror retrieval, but it is never the authoritative Result,
media record, or scientific evidence. A common runner or durable public
artifact wire waits for a second real heavy consumer and the ordinary
duplication and abstraction-budget review. This contract does not choose an
object store, HPC provider, scheduler, archive format, signing scheme,
retention policy, scene graph, remote-execution API, or calendar cadence.
It does not weaken independent-oracle ownership, convergence or refinement
obligations, registered evidence, or the common publication admission
predicate below.

## Publication classes

### Unverified product example

A new public gallery entry may ship as an unverified product example when it supplies:

- a real bounded Model--Plan--Run--Result workflow through the supported product surface;
- focused positive and failure tests for that workflow;
- a reproducible installed-package invocation with named inputs, quantities, and units;
- an explicit **Unverified product example** label and important scientific non-claims; and
- a reduced-motion still and text alternative that communicate the result without the film.

This class is publishable and may use a production-resolution Result. It does not require or
create a registered case, oracle, tolerance, falsifier, evidence dossier, candidate campaign, or
accepted-result digest. It must not use `verified`, `validated`, or equivalent scientific
acceptance language.

### Accepted scientific result

An entry gains accepted-result status when its Model--Realization--Run--Result lineage,
independently derived registered evidence, public claim, non-claims, media integrity records, and
accessibility assets are complete, fixed for the candidate, and reproducible.

Synthetic or incomplete mockups remain visibly marked development previews. They are distinct
from real unverified product examples. A gallery entry never becomes evidence merely because its
media files have digests or looks physically plausible.

The first implementation may keep this delivery record private to the site and
its build. A durable public artifact schema is introduced only when a second
real consumer proves the shared boundary. The normative requirements above do
not depend on that future encoding.

## Result and renderer boundary

The film consumes real Result data; it never reconstructs a scientific trajectory from
storyboard keyframes. An accepted-scientific-result entry may display only its unchanged accepted
unit-bearing projections. An unverified product example may display unit-bearing quantities
produced by its bounded workflow, with the unverified label and non-claims preserved. The
renderer may interpolate colors, place labels, choose a camera, and
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
Run, Result, or trajectory contracts as the product and Studio. It does not
recreate missing meaning in Python. A repository-only Rust executable, private
Studio bridge, or re-authored demo dataset may help development, but does not
complete the workflow. Focused Rust/Python tests cover the product path; they do not become
scientific evidence. Registered evidence may separately support the exact claim.

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

Each published entry provides:

- a lossless poster fallback and, when worthwhile, a modern compressed poster;
- one efficient modern video and one broadly supported fallback with correct MIME types;
- intrinsic dimensions, duration, frame rate, and no layout-shifting load;
- responsive crops that do not remove the decisive observable or unit;
- a text alternative and a short ordered description of the visual changes.

Autoplay is disabled under `prefers-reduced-motion`. The still shown in its place includes the
geometry, primary quantity, physical state, decisive observable, and either the unverified label
or the existing accepted-result link. Color is never the only carrier of state. Keyboard and
screen-reader users can reach the same product details without traversing video controls.

Normal site asset-integrity checks apply to unverified examples without becoming scientific
evidence. Accepted-result media retains its claim-local integrity records.

## Accepted-result dossier

Only an accepted scientific result carries an evidence dossier. Its dossier
distinguishes four evidence kinds rather than collapsing them into one badge:

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

Create or update a dossier only from independently derived, claim-local evidence. An unverified
product example instead publishes its invocation, quantities, units, focused-test boundary, and
explicit scientific non-claims.

## Publication progression

An unverified product example moves through:

1. **contracted** -- this document fixes the public responsibility and
   non-claims;
2. **workflow-ready** -- the installed product workflow and focused tests pass;
3. **result-ready** -- a real bounded Result and displayed unit-bearing quantities exist;
4. **publication-ready** -- the unverified label, non-claims, media, and accessibility assets
   pass ordinary site checks; and
5. **published** -- the example is deployed.

An accepted scientific result retains its accepted evidence-ready, result-ready, and publication
records. A new entry enters that track only after all corresponding acceptance obligations pass.

No earlier state implies a later one. In particular, these future-state
experience specifications do not change any capability or benchmark status.
