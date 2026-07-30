# Benchmark and example roadmap

This catalog records what Eqiora intends to verify over time. It is deliberately
broader than the active implementation roadmap and is not a support matrix.
Each case remains `proposed` until the evidence required by a higher status
exists.

## Why the portfolio is layered

A famous benchmark by itself rarely identifies why a solver is wrong. Each
physics area should therefore develop four complementary kinds of case:

1. **Analytic or manufactured verification** — measures discretization error
   and convergence order against a known solution.
2. **Boundary-condition gallery** — reuses one model while changing constraints,
   loads, sources, or interface conditions.
3. **Numerical stress case** — exposes locking, singularities, discontinuities,
   contact, stiffness, conservation loss, or unstable coupling.
4. **Coupled case** — proves that the same kernel composes with another physics
   or with hybrid-time semantics.

Tutorials optimize for clarity. Verification cases optimize for diagnosis.
Community benchmarks optimize for comparison. Validation cases compare a model
with experimental evidence. One case may serve more than one role, but those
claims must be evaluated separately.

## Status vocabulary

| Status | Required evidence |
|---|---|
| `proposed` | A useful candidate with a stated capability target |
| `specified` | Equations, geometry, data, boundary conditions, quantities of interest, and reference source are fixed |
| `implemented` | The canonical model executes reproducibly and produces the declared quantities |
| `verified` | Declared analytic/manufactured error, applicable convergence and conservation, and tolerance contracts pass in CI; any inapplicable axis is justified |
| `validated` | The verified model is compared with licensed experimental or accepted community evidence within a declared validity range |

Status is monotone only while its evidence remains reproducible. A regression
may demote a case. Merely producing a plausible plot never establishes
`verified` or `validated`.

## Initial implementation sequence

These cases give the smallest balanced path from exact solutions to coupled
semantics.

| Order | ID | Case | Primary evidence target | Capabilities | Status |
|---:|---|---|---|---|---|
| 1 | `solid.axial-bar` | Linear axial bar | Analytic displacement, stress, reaction | units, essential/natural BCs, linear elasticity | `verified` |
| 2 | `numerics.poisson-fem-fvm` | Manufactured scalar Poisson | L2 error, order, global balance for two realizations | canonical coordinate source, FEM/FVM lowering, assembly | `verified` |
| 3 | `numerics.cartesian-poisson-fem-fvm` | 2D manufactured Cartesian Poisson | Continuous L2 error, order, global balance for Q1/TPFA | multidimensional lowering, Cartesian mesh, cell/facet operators | `verified` |
| 4 | `numerics.cartesian-poisson-3d-fem-fvm` | 3D manufactured Cartesian Poisson | Continuous L2 error, order, global balance for Q1/TPFA | dimension-three canonical lowering through the shared spatial path | `verified` |
| 5 | `time.general-implicit-dae` | State-dependent-coefficient semi-explicit DAE | Consistent pair, temporal order, algebraic constraint | residual/JVP lowering, variable partition, nonlinear derivative, reference implicit Euler | `verified` |
| 6 | `solid.beam-bc-gallery` | Cantilever, simply supported, and fixed-fixed beam | Analytic displacement, rotation, reaction, moment, eigenfrequency | reusable model, BC substitution, beam kinematics | `proposed` |
| 7 | `thermal.slab-bc-gallery` | Steady and transient slab with Dirichlet, Neumann, and Robin boundaries | Analytic temperature and heat flux | diffusion, time integration, boundary operators | `proposed` |
| 8 | `fluid.couette-poiseuille` | Couette, plane Poiseuille, and pipe Poiseuille flow | Analytic velocity, flow rate, wall shear, mass balance | viscosity, no-slip, pressure/body-force/flow-rate driving | `proposed` |
| 9 | `hybrid.thermal-sampled-controller` | Continuous thermal plant with sampled controller | Analytic limits and deterministic reference trajectory | continuous/periodic Activation, `pre`/`next`, hold | `implemented` |
| 10 | `fluid.taylor-green` | Taylor–Green vortex | Analytic decay or manufactured reference | smooth transient flow, temporal/spatial convergence | `proposed` |
| 11 | `thermal.scalar-heat-mms` | Scalar heat equation | Manufactured solution and expected convergence order | spatial realization, source terms, default realization | `proposed` |
| 12 | `fluid.lid-driven-cavity` | Lid-driven cavity | Published centerline and vortex quantities plus refinement | incompressibility, recirculation, corner singularity | `proposed` |
| 13 | `fluid.sod` | Sod shock tube | Exact Riemann solution, conservation, discontinuity location | compressible Euler, shock/contact/rarefaction | `proposed` |
| 14 | `hybrid.bouncing-ball` | Bouncing ball | Event time/reset state; temporal convergence and long-time energy pending | zero crossing and atomic reset; first-impact hybrid AD verified separately | `implemented` |
| 15 | `hybrid.packaged-dc-motor-controller` | Exact-packaged sampled acausal DC drive | Matrix-exponential trajectory, backward-Euler refinement, `PhysicalSample` residual and power/energy reacceptance | three exact packages, scalar electrical/rotational conserving domains, one exact clock, output-less Run v1 identity lineage | `verified` |
| 16 | `multiphysics.thermoelastic-bar` | Thermally loaded elastic bar | Analytic free and constrained expansion | field coupling, material parameters, reactions | `proposed` |

The generic numerical precursor `numerics.diffusion1d-mms` has a kernel-level
convergence check in CI:
centered differences and Crank–Nicolson reproduce the analytic decay
`sin(pi x) exp(-alpha pi^2 t)` with measured refinement rates above 1.9. It is
not a canonical Eqiora model, so it does not promote
`thermal.scalar-heat-mms`; canonical transient mass/time semantics and a heat
realization remain required for that case.

The canonical `numerics.poisson-fem-fvm` case is `verified`. One Eqiora source
declares a coordinate-dependent manufactured source, which lowers once before
cell-local FEM and cell/facet-local FVM branch into method-native operators.
Both continuous `L2` reconstructions converge at second order and both global
balance defects remain below their declared tolerance. The reproducible table
and exact claim boundary are in [`poisson-fem-fvm.md`](poisson-fem-fvm.md).
This verifies one scalar 1D cross-method slice, not multidimensional PDE
support.

The independent `numerics.cartesian-poisson-fem-fvm` and
`numerics.cartesian-poisson-3d-fem-fvm` cases are also `verified`. They compile
canonical 2D and 3D boxes and execute each revision through the same Q1 FEM and
orthogonal TPFA FVM path. Both continuous reconstructions converge at second
order and both global balances close within the declared tolerance; see
[`cartesian-poisson-fem-fvm.md`](cartesian-poisson-fem-fvm.md) and
[`cartesian-poisson-3d-fem-fvm.md`](cartesian-poisson-3d-fem-fvm.md). Together
with the 1D case, these promote the scalar Cartesian reference envelope to
`1D..=3D`, but not vector/tensor fields, unstructured/adaptive meshes, or
nonorthogonal FVM.

The first canonical spatial case, `solid.axial-bar`, is `verified`. Eqiora
source declares the interval, boundary Domains, continuum displacement Field,
strong equilibrium, and essential/natural boundary Relations. The default
realization produces P1 FEM local contributions and CI checks analytic tip
displacement, stress on every cell, and clamp reaction. This establishes a
constant-coefficient scalar 1D slice, not multidimensional tensor elasticity.

The registered `language.canonical-tensor-operators` case separately verifies
the typed semantic/source composition and explicit wire-v4 replay of
`symmetric_part(grad(u))` and `isotropic_lift(div(u))`, together with their
exact pointwise component rules. It supplies a canonical-language prerequisite
for multidimensional solids, not a numerical one: it has no boundary-value
problem, discretization, solve, field result, patch test, or convergence
evidence and therefore does not promote any structural-mechanics candidate.

The canonical `time.general-implicit-dae` case is `verified`. A Relation with
a state-dependent derivative coefficient is rejected by the constant
first-order projection, retained as residual plus analytic paired JVP by the
general projection, consistently initialized from an inconsistent algebraic
guess, and advanced by the deterministic implicit-Euler oracle. Terminal
error converges at first order and the algebraic constraint remains at
roundoff. The same target checks that a scalar nonlinear-derivative residual
retains an explicitly supplied branch; see
[`general-implicit-dae.md`](general-implicit-dae.md). Dedicated general-lowering,
supplied/accepted-initial-pair, and run artifacts also round-trip with bounded
decode and linkage-drift rejection. This is one semi-explicit index-one
reference fixture plus one branch check, not automatic branch selection,
production IDA, arbitrary-index analysis, sparse nonlinear algebra, DAE
sensitivity, or hybrid DAE evidence. Residual-native semantic restart lineage
is verified separately and does not widen this numerical-solver claim.

The separate `artifacts.implicit-time-restart-lineage` case records the
accepted point at `t = 0.1`, independently replays its canonical residual,
derives exact `Provided` child initial data, and proves an acyclic parent-output
to child-run edge. Restarted and uninterrupted fixed-step reference runs agree
at `t = 0.2`; value, dimension, output-link, start-time, and cycle drift fail
closed. It does not preserve adaptive-controller, BDF, Newton, or linear-solver
history and is not yet an adjoint trajectory checkpoint contract.

The separate `differentiation.discrete-implicit-step` case freezes one accepted
step of that canonical DAE as
`F(t_next, y_next, (y_next-y_previous)/h, p)=0`. It checks the projected
JVP/VJP duality, a nonsymmetric normal solve, a VJP-backed transposed adjoint,
and forward/adjoint derivatives against centered differences of an independent
closed-form discrete map. It does not claim multi-step reverse accumulation,
adaptive-controller differentiation, BDF history, trajectory adjoints, or
composition of step cotangents across the available semantic checkpoint
lineage.

The canonical `hybrid.bouncing-ball` case is `implemented`. The reference
interpreter localizes a directed height crossing by re-solving bracketed
implicit steps, groups split height/velocity reset Relations, and commits their
`Next` values atomically. Direction filtering, graph-order independence, and a
zero-time chatter safety diagnostic execute in CI. The separate
`differentiation.hybrid-event` and `hybrid.registered-event` cases verify
analytic first-impact derivatives and the content-registered production
proposal/reset/restart path. Backward-Euler trajectory convergence, long-time
energy trend, and mathematical Zeno classification remain unverified, so the
broader trajectory case is not yet promoted to `verified`.

The independent
[`hybrid.packaged-dc-motor-controller`](../../verify/hybrid/packaged-dc-motor-controller/README.md)
case is `verified`. It resolves exactly three ordinary Model Packages into one
Model-v2 flat Relation network, then executes one ideal linear scalar motor and
viscous load across electrical and rotational conserving domains with one
proportional controller and one exact 10 ms clock. Host-serial `f64`
initialization and backward-Euler steps solve a 23-by-23 dense Newton system;
an independent two-state matrix exponential, step refinement, dimensioned
`PhysicalSample` residuals, and electromechanical power/energy balance accept
the reference trajectory. Only then may the case create an output-less
`RunManifestV1` and `PackageRunBindingV1`; this records exact identity lineage,
not execution attestation or a durable trajectory artifact. It does not claim
arbitrary DAE index, switching or saturation, Event/Guard composition,
Simulink, Simscape, Stateflow, production solvers, code generation, fixed
point, real-time scheduling, MPI, GPU, broad component libraries, or dynamic
plugins.

The native Studio packaged DC-drive example does not widen that scientific
claim. `interfaces.studio-packaged-dc-motor-demo` composes the same checked-in
package closure and accepted executor into a bounded current/speed/held-voltage
presentation with exact package, Model, Run, and binding lineage. Studio
recomputes neither reference values nor residual, controller, power, or energy
expressions; those remain solely owned by the hybrid case.

## Structural mechanics candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `solid.patch-test` | Constant-strain patch test | element consistency and coordinate invariance | `proposed` |
| `solid.kirsch-plate` | Plate with a circular hole | stress concentration and curved geometry | `proposed` |
| `solid.cook-membrane` | Cook's membrane | bending-dominated distortion and locking | `proposed` |
| `solid.l-shaped-domain` | L-shaped elastic domain | singular stress and adaptive refinement | `proposed` |
| `solid.nearly-incompressible-block` | Nearly incompressible block | volumetric locking and mixed formulation | `proposed` |
| `solid.large-cantilever` | Large-rotation cantilever | geometric nonlinearity and frame objectivity | `proposed` |
| `solid.euler-buckling` | Column buckling | geometric stiffness and eigenvalue bifurcation | `proposed` |
| `solid.snap-through-arch` | Shallow arch snap-through | path following and limit points | `proposed` |
| `solid.hertz-contact` | Hertz contact | unilateral contact and pressure distribution | `proposed` |
| `solid.free-forced-vibration` | Beam/plate vibration gallery | mass, damping, eigenmodes, transient forcing | `proposed` |

The beam gallery is an educational entry point, not sufficient evidence for a
continuum solid implementation. Patch, locking, singularity, dynamics, and
nonlinear cases are separate obligations.

The native Studio mixed-boundary elastic-panel example does not advance any
candidate in this table. `interfaces.studio-mixed-boundary-elasticity-demo`
composes the already verified `solid.mixed-boundary-elasticity-2d` direct Model
and public Q1/CG executor into a bounded displacement/reaction presentation.
Studio derives no stress, strain, traction, analytic reference, convergence
order, or validation quantity; the existing scientific case remains their sole
authority.

## Incompressible and low-Mach flow candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `fluid.flow-past-cylinder` | Laminar cylinder wake | drag, lift, separation, Strouhal number | `proposed` |
| `fluid.backward-facing-step` | Backward-facing step | recirculation and reattachment length | `proposed` |
| `fluid.natural-convection-cavity` | Differentially heated cavity | momentum/energy coupling and Nusselt number | `proposed` |
| `fluid.oseen-mms` | Manufactured Oseen/Navier–Stokes flow | pressure/velocity convergence and inf-sup behavior | `proposed` |
| `fluid.hydrostatic-balance` | Static fluid under gravity | pressure null space and well-balanced forcing | `proposed` |

The native Studio `steady-flow-past-cylinder` example does not advance
`fluid.flow-past-cylinder`: it is a bounded steady Stokes demonstration on one
coarse error-controlled affine mesh, with no Reynolds-number, wake,
drag/lift-coefficient, Strouhal-number, convergence, benchmark, or validation
claim. Its reproducible application-composition evidence is indexed separately
as `interfaces.studio-exact-cylinder-stokes-demo`.

## Compressible-flow candidates

Smooth and discontinuous cases are intentionally separate: a method can be
high-order on smooth fields and still put shocks at the wrong location.

| ID | Case | Purpose | Status |
|---|---|---|---|
| `fluid.linear-advection` | Periodic linear advection | dispersion, dissipation, formal order | `proposed` |
| `fluid.isentropic-vortex` | Smooth isentropic vortex | multidimensional high-order convergence | `proposed` |
| `fluid.burgers-shock` | Burgers shock formation | nonlinear steepening and entropy solution | `proposed` |
| `fluid.lax-shock-tube` | Lax Riemann problem | alternate shock/contact structure | `proposed` |
| `fluid.shu-osher` | Shock–entropy-wave interaction | resolution without spurious oscillation | `proposed` |
| `fluid.double-mach-reflection` | Double Mach reflection | multidimensional shock interactions | `proposed` |
| `fluid.noh-implosion` | Noh implosion | symmetry, strong shock, wall heating error | `proposed` |
| `fluid.shock-boundary-layer` | Shock/boundary-layer interaction | viscous-compressible coupling | `proposed` |

Every discontinuous case must report conservation error, discontinuity-location
error, overshoot/undershoot, and a norm appropriate to nonsmooth solutions.

## Thermal candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `thermal.composite-wall` | Multi-material wall | interface continuity and discontinuous conductivity | `proposed` |
| `thermal.internal-generation` | Conduction with internal heat generation | volumetric sources and energy balance | `proposed` |
| `thermal.anisotropic-block` | Anisotropic conduction | tensor coefficients and frame transforms | `proposed` |
| `thermal.stefan` | One-phase/two-phase Stefan problem | moving interface and latent heat | `proposed` |
| `thermal.radiative-enclosure` | Radiative enclosure | nonlocal boundary coupling | `proposed` |

## Electromagnetics and acoustics candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `em.parallel-plate-capacitor` | Parallel-plate capacitor | electrostatic potential, energy, capacitance | `proposed` |
| `em.coaxial-cable` | Coaxial cable | analytic field, material interface, impedance | `proposed` |
| `em.current-wire` | Current-carrying wire | magnetostatic field and orientation | `proposed` |
| `em.rectangular-waveguide` | Rectangular waveguide | eigenmodes and cutoff frequency | `proposed` |
| `em.resonant-cavity` | Electromagnetic cavity | vector eigenproblem and spurious-mode control | `proposed` |
| `em.skin-effect` | Conducting slab/cylinder | frequency-domain diffusion and skin depth | `proposed` |
| `em.team-7` | TEAM Problem 7 | 3D eddy-current community comparison | `proposed` |
| `em.mie-sphere` | Scattering from a dielectric/conducting sphere | open boundary and analytic scattering coefficients | `proposed` |
| `acoustics.duct-standing-wave` | Standing wave in a duct | wave propagation and reflecting boundaries | `proposed` |
| `acoustics.helmholtz-cavity` | Helmholtz cavity modes | scalar eigenproblem | `proposed` |

## Porous, multiphase, particle, and reactive candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `porous.darcy-mms` | Manufactured Darcy flow | mixed flux/pressure convergence and local conservation | `proposed` |
| `porous.radial-well` | Radial well flow | logarithmic solution and source singularity | `proposed` |
| `porous.buckley-leverett` | Buckley–Leverett displacement | nonlinear saturation shock | `proposed` |
| `multiphase.dam-break` | Dam break | free surface, mass conservation, experimental comparison | `proposed` |
| `multiphase.rising-bubble` | Rising bubble | interface curvature and force balance | `proposed` |
| `multiphase.rayleigh-taylor` | Rayleigh–Taylor instability | unstable interface growth | `proposed` |
| `particles.two-body-collision` | Two-particle collision | contact impulse and restitution | `proposed` |
| `particles.granular-column` | Granular column collapse | frictional contact and runout | `proposed` |
| `chemistry.first-order-decay` | First-order reaction | exact kinetics and unit handling | `proposed` |
| `chemistry.robertson` | Robertson kinetics | stiff ODE integration and positivity | `proposed` |

## Hybrid, multiphysics, optimization, and multiscale candidates

| ID | Case | Purpose | Status |
|---|---|---|---|
| `hybrid.multirate-algebraic-loop` | Multi-rate feedback with algebraic loop | clock propagation, rate transition, implicit SCC | `proposed` |
| `hybrid.thermostat` | Thermostat with hysteresis | guard, event ordering, mode persistence | `proposed` |
| `hybrid.inverted-pendulum` | Controlled inverted pendulum | nonlinear plant and sampled control | `proposed` |
| `hybrid.fault-thermal-plant` | Fault statechart with thermal plant | run-to-completion and fault reset | `proposed` |
| `multiphysics.joule-heating` | Electric conduction with Joule heating | field transfer and nonlinear material feedback | `proposed` |
| `multiphysics.conjugate-heat-channel` | Fluid/solid conjugate heat transfer | interface flux conservation | `proposed` |
| `multiphysics.turek-hron-fsi` | Turek–Hron FSI | moving interface, lift/drag, partitioned/monolithic coupling | `proposed` |
| `multiphysics.induction-heating` | Eddy currents with thermal response | harmonic/transient coupling | `proposed` |
| `multiphysics.piezoelectric-beam` | Piezoelectric beam | electromechanical constitutive coupling | `proposed` |
| `optimization.cantilever-compliance` | Cantilever compliance optimization | gradient and topology/shape update | `proposed` |
| `optimization.cylinder-drag` | Cylinder drag reduction | PDE-constrained shape derivative | `proposed` |
| `inverse.transient-heat` | Infer conductivity/source from temperatures | parameter sensitivity and uncertainty | `proposed` |
| `multiscale.periodic-cell` | Periodic homogenization cell | periodic BC and effective tensor | `proposed` |
| `multiscale.fe2-bar` | FE² bar/material point | nested solve, batching, surrogate fallback | `proposed` |

## Case contract

An implemented case lives below `verify/` and keeps human explanation separate
from the machine-readable acceptance contract:

```text
verify/<area>/<case>/
├── case.toml
├── README.md
├── models/
├── references/
└── expected/
```

The repository runner validates every `case.toml`. A `verified` case must name
a structured target from the closed shell-free runner set. Most numerical
cases use a Cargo integration-test target:

```toml
id = "fluid.poiseuille"
status = "verified"
reference_kind = "analytic"

capabilities = [
  "incompressible",
  "viscous-flux",
  "dirichlet-bc",
  "pressure-driving",
  "conservation",
]

[quantities.velocity_l2]
tolerance = 1e-8

[quantities.mass_imbalance]
tolerance = 1e-12

[quantities.wall_shear]
relative_tolerance = 1e-6

[convergence]
norm = "L2"
minimum_order = 1.9

[evidence]
package = "eqiora-numerics"
test = "canonical_axial_bar"
```

The command and machine-report contract is documented in the
[verification runner guide](runner.md).

Concrete tolerances above are illustrative until the case is `specified`.
Specifications must distinguish discretization error, iterative tolerance,
floating-point/backend variation, and uncertainty in external reference data.

## Reference and licensing policy

- Cite the original analytic derivation, benchmark publication, or experimental
  dataset rather than an uncited value copied from another code.
- Keep third-party data out of the repository unless its license permits
  redistribution; otherwise provide acquisition instructions and checksums.
- Record normalization, nondimensionalization, geometry conventions, and
  quantity definitions. Matching a number under a different convention is not
  verification.
- Preserve raw reference data and derived comparison data as separate artifacts
  with provenance.

## Promotion rule

A roadmap entry advances only when its current evidence is reviewable. New
physics support should normally add one analytic/manufactured case before a
large showcase. No benchmark may require an example-specific Semantic Kernel
node; differences must be expressed through relations, activations,
connections, clocks, ontology views, and realizations.
