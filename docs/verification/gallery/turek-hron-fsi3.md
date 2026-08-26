# Turek--Hron FSI3 experience

Implement and publish this first as an **Unverified product example**. Promote it only after its
claim-local candidate, independent oracle, falsifiers, and acceptance requirements pass under
RFC 0089; evidence work must not block the earlier product path.

Status: future-state experience contract. It does not advance
`multiphysics.turek-hron-fsi`.

## Responsibility and public claim

The film shows bidirectional interaction between the laminar wake of a fixed
cylinder and its attached elastic flag in the Turek--Hron FSI3 numerical
benchmark. One accepted lineage owns fluid and solid states, moving mesh,
conservative interface exchange, and the reported observables.

This is the gallery's coupling signature. It claims comparison with a
published numerical benchmark, not experimental validation. The retained FSI3
case uses the benchmark's finite-strain solid model; substituting the current
linear solid and keeping the FSI3 name is forbidden.

The film does not claim general FSI, partitioned and monolithic equivalence,
production preconditioning, remeshing, turbulence, or three-dimensional
coupling.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Fluid and solid domains, inlet, wall/outlet conditions, material properties, interface, and point A |
| 2--12 s | Vorticity is the primary field; the accepted deformed interface and mesh motion remain visible without a second color field |
| 12--15 s | Point-A displacement trace and one interface balance defect mark the current phase |
| 15--18 s | Phase-matched return after a complete accepted oscillation |

The fixed camera makes the flag amplitude readable. Mesh lines appear only in
a short, labelled interval if needed to explain ALE motion.

## Accepted-result evidence plan

The coupled case consumes already accepted transient non-box flow, finite-
strain structure, moving-mesh/GCL, and interface-transfer evidence. The
scientific case then compares point-A horizontal and vertical displacement
mean, amplitude, and frequency, together with drag and lift, in the source
conventions. Fluid mass, solid energy, geometric conservation, and interface
action/power defects remain separate dossier quantities.

The decisive falsifier is the complete point-A and force observable family
under spatial and temporal refinement. Failure of any precommitted component
rejects the case even when the phase portrait looks plausible. A linear-solid
sensitivity run is a diagnostic, not a substitute oracle; if it is
indistinguishable within the accepted band, the case specification or band
must be returned for review before implementation continues.

Derivable interface, energy, and manufactured component checks use dual
independent oracles. Published FSI3 comparison bands are extracted and
normalised independently of the implementer.

## Capability and artifact dependencies

- the accepted cylinder-wake fluid seam, including force projection;
- finite-strain St. Venant--Kirchhoff solid dynamics;
- fixed-topology ALE mesh motion and geometric conservation on this exact
  geometry;
- conservative coupled iteration with independently visible fluid, solid, and
  interface acceptance;
- durable coupled trajectories and synchronized field/trace playback.

The experience does not require a general FSI visualization protocol. It
extends the accepted gallery media boundary only for one moving 2D interface
and one coupled observable group.

## Accessibility and promotion

The reduced-motion still shows the maximum accepted point-A displacement,
vorticity, interface direction, displacement amplitude/frequency, and balance
route. The text alternative explains how the wake and flag motion exchange
energy.

Promotion requires all component cases, accepted FSI3 refinement and
comparison evidence, a coupled trajectory whose displayed fields share exact
state identities, and common publication admission.

## Primary source

S. Turek and J. Hron,
[“Proposal for Numerical Benchmarking of Fluid-Structure Interaction between an Elastic Object and Laminar Incompressible Flow”](https://doi.org/10.1007/3-540-34596-5_15),
in *Fluid--Structure Interaction*, LNCSE 53, 2006.
