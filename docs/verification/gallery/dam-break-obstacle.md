# Dam-break-around-an-obstacle experience

Status: future-state experience contract. It does not advance
`multiphase.dam-break`.

## Responsibility and public claim

The film shows a gravity-driven free surface released in the Kleefsman et al.
dam-break geometry, impacting the fixed obstacle and producing accepted water-
height and pressure-probe histories. The fluid state, interface
representation, obstacle load, and conservation diagnostics share one result
lineage.

The public claim is a verified interface-capturing implementation plus a
comparison with the selected experiment within its declared measurement and
model uncertainty. It does not claim a resolved air phase unless the retained
model solves it, universal impact-pressure accuracy, cavitation, compressible
impact, arbitrary fragmentation, or general free-surface CFD.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Tank, initial water column, gravity, obstacle, gauges, pressure sensors, and selected one- or two-fluid model |
| 2--12 s | Water volume fraction or level-set field is the sole primary field through release, impact, run-up, and bounded rebound |
| 12--15 s | One height gauge and one obstacle-pressure trace mark arrival and peak windows; mass defect remains visible |
| 15--18 s | Labelled neutral reset to the initial column; impact is never reversed |

The camera is fixed to preserve gauge and obstacle location. Any interface
threshold used for the visible surface is recorded and fixed.

## Evidence and falsifiers

The prerequisite ladder verifies hydrostatic balance, interface advection,
boundedness, mass conservation, surface reconstruction, gravity forcing, wall
conditions, and pressure/force projection before experimental comparison.
Spatial, temporal, interface-thickness, and sensor-sampling effects are
separated.

The decisive observable family is free-surface height at the selected gauges,
obstacle-face pressure at the selected sensors, first-impact arrival time, and
total liquid-volume defect. The experience is rejected if arrival time misses
its pre-registered experimental band or if mass defect exceeds its independently
derived budget. A matching single pressure spike or visually plausible splash
cannot compensate.

Hydrostatic, transport, and conservation identities use dual independent
oracles. Experimental curves, sensor response, extraction, and uncertainty are
owned by an independent evidence lane and retain the source's conventions.

## Capability and artifact dependencies

- one explicitly selected VOF, level-set, or other interface-capturing model;
- gravity, pressure, interface transport, boundedness, and free-surface
  boundary semantics;
- obstacle pressure and force result projections with sensor provenance;
- accepted adaptive or fixed trajectories, including conservative transfer if
  the mesh changes;
- synchronized interface, gauge, and pressure playback.

The model choice is frozen before implementation. This experience is not
permission to introduce multiple interchangeable multiphase formulations in
one slice.

## Accessibility and promotion

The reduced-motion still shows the accepted impact state, original column
outline, gauge/pressure locations, arrival time, mass defect, and evidence
route. The text alternative describes the interface position and obstacle
impact without relying on transparency or color.

Promotion requires accepted hydrostatic, transport, conservation, sensor, and
experimental-comparison evidence; an accepted free-surface trajectory; and
common publication admission.

## Primary source

K. M. T. Kleefsman et al.,
[“A Volume-of-Fluid based simulation method for wave impact problems”](https://doi.org/10.1016/j.jcp.2004.12.007),
*Journal of Computational Physics* 206, 2005. Reference-data redistribution
requires an explicit licence; otherwise the evidence package records
acquisition instructions and checksums.
