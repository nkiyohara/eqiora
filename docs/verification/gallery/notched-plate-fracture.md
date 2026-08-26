# Notched-plate phase-field fracture experience

Implement and publish this first as an **Unverified product example**. Promote it only after its
claim-local candidate, independent oracle, falsifiers, and acceptance requirements pass under
RFC 0089; evidence work must not block the earlier product path.

Status: future-state experience contract. It does not advance a fracture
capability or benchmark.

## Responsibility and public claim

The film shows quasi-static propagation from one pre-existing notch in a
two-dimensional brittle plate using one explicitly selected phase-field
fracture formulation. The accepted load path, displacement, phase field,
irreversibility state, and energy terms share one lineage.

The first retained case demonstrates initiation and propagation, not dynamic
branching. It claims the exact regularized model, length scale, mesh relation,
loading protocol, irreversibility treatment, and energy evidence selected by
the scientific case. It does not claim a sharp crack at finite length scale,
mesh-independent crack topology, material validation, fatigue, ductile
fracture, or general topology change.

## Storyboard

| Presentation time | Content |
|---|---|
| 0--2 s | Plate, notch, supports, imposed displacement, elastic and fracture parameters, phase-field length scale, and mesh relation |
| 2--11 s | Phase field is the sole primary field as the accepted load steps cross initiation and propagate the regularized crack |
| 11--15 s | Load--displacement and elastic/fracture energy histories mark peak load and current state |
| 15--18 s | Labelled neutral reset; damage never heals or plays backward |

Displacement may be geometrically shown at a fixed labelled scale, but it does
not become a simultaneous color field.

## Accepted-result evidence plan

The prerequisite ladder verifies elasticity, phase-field regularization,
variational derivatives, the coupled/staggered solve actually selected,
irreversibility, and mesh-to-length-scale convergence. The dossier separates
model regularization error from spatial, nonlinear-solve, and load-step error.

The decisive observable family is load versus imposed displacement, peak load,
elastic and fracture energies, irreversibility defect, and regularized crack
surface measure. The experience is rejected if damage decreases at any
accepted material point or if the fracture-energy relation fails its
precommitted refinement check at fixed model length scale. A visually correct
crack path cannot override either failure.

Variational derivatives, one-dimensional regularization profiles, and energy
identities use dual independent oracles. Published notched-plate curves are a
separate community comparison unless licensed experimental data and a material
calibration protocol are fixed.

## Capability and artifact dependencies

- an immutable phase-field fracture model with explicit tension/compression
  split and irreversibility choice;
- accepted nonlinear or staggered solve semantics and load stepping;
- mesh/length-scale admission and adaptivity only when its transfer preserves
  irreversibility and energy accounting;
- durable displacement, phase, history, and energy trajectories;
- synchronized 2D field and load-path playback.

The first case keeps one notch and one propagation mode. Branching, XFEM,
cohesive zones, remeshing, and finite-strain fracture remain separate slices.

## Accessibility and promotion

The reduced-motion still shows the accepted post-initiation phase field,
notch/crack direction, peak load, energy values, and evidence route. The text
alternative states that the diffuse band represents a regularized crack and
names the selected length scale.

Promotion requires accepted regularization, derivative, irreversibility,
energy, refinement, and notched-plate evidence; an accepted coupled
trajectory; and common publication admission.

## Primary source

C. Miehe, F. Welschinger, and M. Hofacker,
[“Thermodynamically consistent phase-field models of fracture”](https://doi.org/10.1002/nme.2861),
2010. The exact example, split, and irreversibility algorithm are fixed by the
scientific child specification before implementation.
