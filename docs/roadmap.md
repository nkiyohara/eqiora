# Roadmap

Eqiora advances by dependency-closed vertical slices, not by calendar phases.
Each capability must travel through:

```text
typed meaning
  -> lowered contract
  -> Realization or adapter
  -> falsifier
  -> registered evidence
  -> capability-matrix update
```

The manifests under [`verify/`](../verify/) are the authority for executable
claims. The [capability matrix](capability-matrix.md) is their whole-product
index. This roadmap records dependency order and deliberate nonclaims; it does
not widen either source.

## Multiphysics dependency spine

The main multiphysics line has one semantic spine. Every item shown as closed
has a typed implementation and bounded registered evidence; “closed” never
means general production coverage.

```text
Geometry identity and mesh correspondence                         closed
  -> fixed-reference monolithic 2D FSI on the CPU                 closed
  -> version-neutral Model identity and replay                    closed
  -> durable FieldSnapshot / SpatialState / SpatialTrajectory     closed
  -> bounded CAD semantic selection                               closed
  -> physics-neutral discrete block system                        closed
  -> curated facade and compile/check control plane               closed
  -> identity-preserving Component Parameter terms                closed
  -> proof-carrying pure calculus and support maps                 closed
  -> canonical pure-operator definitions                          closed
  -> fixed-domain transient CFD reference                         closed
  -> portable Realization and bound execution graphs              closed
       |-> conservative cell-centered transport                   closed
       |    -> spatial-periodic Cartesian transport               closed
       |    -> collocated incompressible finite volume            closed
       `-> fixed-mesh execution fork
            |-> single-device CUDA FSI                            closed
            |-> distributed mesh ownership and assembly          closed
            |    `-> MPI fixed-mesh FSI                           closed
            `-> host-staged MPI plus CUDA FSI                     closed
                 `-> fixed-topology ALE FSI
                      |-> bounded 2D reference                     closed
                      |-> bounded tetrahedral 3D reference         closed
                      `-> remeshing correspondence and transfer   closed
                           |-> XDMF/HDF5 trajectory export         closed
                           `-> derived ML Dataset                  closed
```

The owning decisions are [RFC 0037](../rfcs/0037-version-neutral-model-artifact-reference.md)
and [RFC 0049](../rfcs/0049-geometry-identity-and-mesh-correspondence.md)
through [RFC 0067](../rfcs/0067-derived-ml-dataset.md), with the dimension-
parametric 3D extension in
[RFC 0070](../rfcs/0070-dimension-parametric-tetrahedral-ale-fsi.md) and the
finite-volume extensions in
[RFC 0069](../rfcs/0069-conservative-cell-centered-transport.md),
[RFC 0071](../rfcs/0071-spatial-periodic-boundary-connection.md), and
[RFC 0072](../rfcs/0072-collocated-incompressible-finite-volume.md).

The CUDA path consumes the CPU-finalized operator; it does not own a second FSI
lowering. The MPI path consumes accepted owner-row assembly payloads; it does
not reinterpret mesh ownership. MPI plus CUDA is composition evidence over
those two parent paths, not another physical semantics.

### What this spine proves

- One fixed-reference two-dimensional fluid/linear-solid interface can lower
  to a gauge-free monolithic backward-Euler step with exact semantic,
  geometry, mesh, Realization, and Run lineage.
- The same finalized FSI meaning can execute through bounded CPU,
  single-device CUDA, one-host MPI, and host-staged MPI-plus-CUDA paths.
- Fixed-topology ALE derives mesh motion and geometric conservation data rather
  than accepting them as unrelated input.
- A bounded remeshing transition preserves semantic geometry, transfers fields
  through typed field-aware projections, reconstructs target geometry, and
  enters the unchanged target ALE finalizer.
- One accepted remeshing-aware trajectory projects to a complete XDMF 3
  temporal collection and HDF5 file image, and to a separate derived Dataset.

### What it does not prove

- finite-strain structure, contact, turbulence, compressible flow, free
  surfaces, general nonmatching FSI, or production multiphysics libraries;
- production preconditioners, scale, performance portability, multi-GPU,
  GPU-aware MPI, distributed ALE/remeshing, recovery, or fault semantics;
- arbitrary CAD, persistent naming across topology change, healing, curved or
  high-order geometry, or a general Boolean/history kernel;
- ALE, remeshing, FSI, or CAD shape sensitivities and adjoints;
- temporal XDMF import, arbitrary trajectory export, parallel HDF5, or a
  production Dataset loader.

Those remain independent vertical slices. They must reuse the established
meaning-to-evidence path rather than introduce a second physical or identity
authority.

## Implemented foundations

The following foundations are implemented for the exact boundaries indexed by
the capability matrix.

### Meaning, language, packages, and artifacts

- A small typed Semantic Kernel with inspectable residual-expression DAGs,
  explicit activation, typed Ports, and signal versus conserving connection
  semantics.
- Lossless source lexing, recovering parsing, formatting, typed lowering, and
  canonical Model envelopes with bounded decoding and content identity.
- Deterministic Component expansion, exact-package identity and offline
  resolution, compiler-owned package validation, capability-rooted directory
  admission, retained local-store replay, and atomic no-clobber installation.
- Occurrence-bound spatial supports and Fields, field-valued boundary
  interfaces, complete exterior Port families, and hierarchical conserving
  connection-set normalization.
- Versioned Model, Realization, Run, spatial-state, trajectory, import-lineage,
  and package-execution artifacts. Each wire owns only its declared identity
  and provenance boundary.
- A structural semantic fingerprint for comparing accepted graphs across
  source and native authoring without replacing exact Model artifact identity.

Packages still do not imply a dynamic plugin ABI, online registry, range
solver, public trust service, or general package-manager UX.

### Spatial numerics and physics

- Runtime-dimensional simplex and hypercube topology, affine geometry,
  quadrature, P0/P1/Q1 and bounded MINI spaces, entity-local work, ordered
  assembly, and constraint projection.
- Canonical scalar elliptic Models realized by FEM and FVM in one, two, and
  three dimensions with convergence and balance evidence.
- Bounded imported affine-simplex, Gmsh MSH 4.1, VTU, XDMF, and native HDF5
  paths behind format-specific adapters and shared accepted mesh/Field
  contracts.
- Bounded packages and Realizations for linear elasticity, steady
  incompressible Newtonian flow, mixed boundary data, conforming elastic
  interfaces, first-order dynamic-solid meaning, transient fixed-domain flow,
  and the FSI spine above.
- A conservative cell-centered transport path, a collocated incompressible
  finite-volume path, and a same-Program FEM/FVM comparison at a shared
  analytic equilibrium.

General unstructured/adaptive production meshing, high-order and mixed finite
elements, broad constitutive/component libraries, production CFD, and
industrial-scale validation remain open.

### Execution and differentiation

- Reference scalar continuous, exact-periodic, and bounded zero-crossing/reset
  semantics.
- Serial and threaded CPU execution, faer adapters, explicit true-residual
  acceptance, stable identity/Jacobi preconditioning, and ordered reproducible
  reference reductions.
- Typed distributed ownership, halo exchange, one-host MPI execution, and a
  bounded physical two-node lower-level observation.
- Explicit device, residency, queue, transfer, completion, and receipt
  contracts with bounded CUDA execution.
- Primal, JVP, and VJP actions from one scalar Operator IR; normal and
  transposed solves; smooth implicit sensitivities; one discrete implicit-step
  adjoint; bounded spatial coefficient/shape derivatives; and one
  transversal-event saltation slice.

General DAE execution, simultaneous hybrid event ordering, statechart history,
Zeno policy, adaptive/BDF trajectory adjoints, checkpoint scheduling, FSI/ALE
sensitivities, stronger preconditioners, and production distributed/device
execution remain open.

## Public clients

Client surfaces are projections over the same accepted model and execution
contracts. They do not own alternative semantics.

### Python

Implemented and installed-wheel verified for the bounded Linux x86-64
candidate:

- source compilation, immutable native scalar/spatial authoring, exact Model
  and child-revision identity, and structured diagnostics;
- typed scalar-elliptic FEM/FVM Realization preview and host-serial sync/await
  Run lifecycle with bounded progress and safe-point cancellation;
- immutable CPU `float64` arrays with explicit NumPy copy policy and bounded
  CPU DLPack producer/consumer contracts;
- Parameter-point primal/JVP/VJP evaluation;
- optional bounded PyTorch and JAX CPU differentiation adapters; and
- a self-contained source distribution that rebuilds the declared CPython
  wheel family and verifies installed artifacts outside the checkout.

The alpha release gate adds public package metadata, exact release artifacts,
TestPyPI installation, and production publication only after those same
artifacts pass the release checklist.

GPU DLPack, framework GPU execution, broader transformations, free-threaded
Python, other operating systems and architectures, general graph/PDE authoring,
durable output artifacts, production cancellation, and a broad async API
remain open.

### Studio

Implemented bounded slices include:

- accessible Relation projection, source-span diagnostics, compile/check,
  exact reference-plan preview, controlled reference execution, result
  inspection, and local workspace persistence;
- scalar Field and Parameter edits with revision/value preconditions and
  immutable undo/redo navigation;
- a scalar-elliptic FEM/FVM surface with bounded serial/Rayon placement;
- one CAD box semantic-selection workflow;
- a closed typed workflow registry used by navigation, toolbar, and command
  palette; and
- an explicit data plane for one accepted generated-Cartesian 2D scalar Field,
  synchronized across table, raster, and inspector views.

General mesh rendering, imported/adaptive fields, vector/tensor and 3D
visualization, production level-of-detail, source-language services, dynamic
workflow plugins, and whole-product localization remain open.

## Next dependency-safe slices

Release work and product work remain separate.

1. **Public alpha release.** Close repository hygiene, public documentation,
   release trust, exact candidate artifacts, and TestPyPI installation before
   promoting the same approved artifacts.
2. **Public feedback and falsifiers.** Treat alpha use as input to narrow
   vertical slices; do not widen claims merely because an API is visible.
3. **Geometry and CAD depth.** Extend import, sketch/constraint, feature, and
   topology-tracking capability only through the existing Geometry Identity
   seam.
4. **Transient and moving-domain depth.** Add temporal import, broader
   transient CFD, ALE/remeshing policies, and sensitivities in that dependency
   order.
5. **Scale and portability.** Add production preconditioners, NUMA,
   distributed assembly maturity, multiple accelerators, checkpoint/restart,
   and additional release platforms only with environment-specific evidence.
6. **Libraries and authoring.** Grow reusable physics packages, language/LSP
   support, Python authoring breadth, and Studio visualization without moving
   numerical method or deployment policy into semantic packages.

## Canonical conformance set

Four compact models remain the cross-cutting semantic bar:

1. multi-rate feedback with an algebraic loop;
2. bouncing ball with zero crossing and reset;
3. acausal DC motor with a discrete controller; and
4. fault statechart with a thermal plant.

The bounded packaged DC-motor/controller reference case exists. None of these
cases may introduce example-specific Kernel nodes, and together they still do
not constitute general Simulink, Simscape, Stateflow, real-time code
generation, or production hybrid-system coverage.

The broader analytic, stress, community, and coupled benchmark portfolio is
maintained in the
[benchmark roadmap](verification/benchmark-roadmap.md). An entry there is not
a support claim; its status records the evidence actually available.

See [GOVERNANCE.md](../GOVERNANCE.md), the [RFC index](../rfcs/README.md), and
the [vertical-slice guide](development/vertical-slice-development.md) for how
this roadmap changes.
