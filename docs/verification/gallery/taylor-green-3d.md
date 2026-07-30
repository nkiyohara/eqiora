# Three-dimensional Taylor--Green breakdown experience

Status: future-state experience contract. It is distinct from and does not
advance the analytic or manufactured `fluid.taylor-green` candidate.

## Responsibility and public claim

The film shows a fixed three-dimensional viscous Taylor--Green initial
condition producing vortex sheets, small scales, and peak dissipation in a
triply periodic domain. The accepted run publishes the kinetic-energy,
enstrophy, and dissipation histories and one predeclared three-dimensional
vortex-structure projection.

The public claim is a resolved numerical comparison at one fixed Reynolds
number and resolution envelope. It is not an exact solution, proof of general
DNS accuracy, turbulence-model validation, singularity study, or production
HPC performance claim. The two-dimensional analytic-decay case is a
prerequisite code-verification slice, not the three-dimensional flagship.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Periodic cube, initial velocity symmetries, Reynolds number, grid/order, and physical interval |
| 2--12 s | Fixed or slowly prescribed camera shows one accepted Q-criterion or vorticity-magnitude surface family through peak dissipation |
| 12--15 s | Kinetic energy and dissipation histories mark the current time and accepted peak |
| 15--18 s | Labelled neutral cut to the initial symmetric state; decay is never reversed |

The isovalue and color scale are fixed for the film. Camera, clipping, and
opacity are recorded in the scene profile and cannot change in response to
individual frames.

## Evidence and falsifiers

The evidence ladder contains a smooth two-dimensional analytic/manufactured
Taylor--Green case, three-dimensional periodic operator checks, spatial and
temporal refinement, symmetry checks, and a published-history comparison at
the selected Reynolds number. Distributed/device execution, if shown or
claimed, must reproduce the accepted result within its own deterministic
contract.

The decisive observable family is kinetic energy `E(t)`, enstrophy, viscous
dissipation, and the time and value of peak dissipation. The run is rejected if
the discrete energy identity relating energy decay and viscous dissipation
exceeds its independently derived budget anywhere in the declared interval.
A matching peak value or striking Q-criterion image cannot compensate for
excess numerical dissipation.

Analytic initial integrals, symmetry, and energy identities use dual
independent oracles. Published time histories are extracted and normalised by
an evidence owner independent of the implementation.

## Capability and artifact dependencies

- three-dimensional triply periodic incompressible Navier--Stokes;
- the discretization order and resolution needed by the fixed claim;
- accepted 3D trajectories, large-result projection, and bounded
  distributed/device execution if part of the claim;
- deterministic 3D surface extraction, camera, clipping, and publication.

Three-dimensional output and rendering are their own bounded slices. The film
does not justify a general volume viewer, general turbulence path, or
unbounded result scale.

## Accessibility and promotion

The reduced-motion still shows the accepted peak-dissipation state, a section
or silhouette that remains readable without depth motion, and the full energy
history. Its text alternative describes symmetry loss and scale generation
without treating the isosurface as a quantitative oracle.

Promotion requires the analytic precursor, 3D periodic and refinement
evidence, energy-identity acceptance, published-history comparison, an
accepted 3D result projection, and common publication admission.

## Primary source

M. E. Brachet et al.,
[“Small-scale structure of the Taylor--Green vortex”](https://doi.org/10.1017/S0022112083001159),
*Journal of Fluid Mechanics* 130, 1983.
