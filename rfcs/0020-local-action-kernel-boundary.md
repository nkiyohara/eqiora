# RFC 0020: Device-neutral local-action kernel boundary

- Status: Implemented IR; CubeCL not graduated
- Authors: Eqiora contributors
- Created: 2026-07-18

## Summary

Eqiora lowers one resolved entity-local linear operator to an owned,
shape-homogeneous `LocalLinearActionIr`. The IR has an auditable ordered CPU
evaluator and can be consumed by isolated accelerator experiments. Mesh
identity, global numbering, gather/scatter, runtime handles, and third-party
kernel types do not enter the IR.

The first CubeCL experiment does not graduate. CubeCL 0.10.0 omits ordinary
`f64` buffer/arithmetic capability from the type set shared by its CUDA and
HIP C++ backends. Its dependency graph also requires Rust 1.92 while Eqiora's
production MSRV remains 1.89. A physical CUDA run therefore fails closed at
capability negotiation before allocation. No cross-vendor matrix-free support
claim follows from the existence of the experiment.

## Motivation

Matrix-free execution is not a reason to make mesh, physics, and device
runtime one abstraction. The reusable seam is the anonymous local action
after approximation choices are resolved and before global ownership is
applied:

```text
local operator + geometry + quadrature
                  |
                  v
       LocalLinearActionIr
         /               \
 ordered CPU oracle   backend adapter
         \               /
      independent numerical comparison
```

This leaves global gather/scatter, constraints, partition ownership, and
reduction as separate execution contracts. It also lets Eqiora reject an
accelerator library without discarding the lowered contract.

## Contract

`LocalLinearActionIr` stores one nonempty uniform batch in entity-major,
row-major order. Packed inputs and outputs use the same entity order. Both
local dimensions and every coefficient are validated at construction.

Heterogeneous cells do not force ragged offsets and dynamic branches into the
first kernel. A lowerer emits an ordered collection of uniform batches, one
per admitted element/operator shape. Global identity is supplied by a
separate gather/scatter plan; it is not inferred from batch position.

The reference evaluator assigns one output to one deterministic nested loop:
entity, row, then ascending column. It uses separate multiplication and
addition. Accelerated adapters may offer:

- `Ordered`: the same output ownership and ascending local-column loop; or
- `Fast`: the same mathematical map with explicitly enabled backend fast
  floating-point transformations.

`Ordered` does not promise cross-architecture bit identity until compiled
instruction behavior and kernel identity are part of accepted evidence.

## First lowering

The existing runtime-dimensional Cartesian Q1 diffusion cell operator lowers
to this IR in dimensions one through three. Its source term is intentionally
excluded because a linear action represents the bilinear operator, not the
right-hand side. Inputs remain cell-local Q1 values. Global essential
constraints and scatter are orthogonal.

The registered host conformance path still gathers, executes
`LocalLinearActionIr::apply_reference`, and scatters in dimensions one through
three. Its downstream host composition is now also checked through RFC 0018's
packet action and a separately assembled CSR oracle; see
[`numerics.global-matrix-free-action`](../verify/numerics/global-matrix-free-action/README.md).
The local IR continues to own no global identity or mapping, and the host
packet system is not a device gather/scatter plan.

## CubeCL experiment

The adapter lives in `experiments/cubecl-local-action`, which is a distinct
Cargo workspace. Its dependency and lockfile cannot alter the production
workspace, default facade, MSRV build, or public API. It accepts only
`LocalLinearActionIr` and records the Eqiora kernel-contract version, CubeCL
version, runtime, selected policy, output comparison, and phase timings.

CubeCL remains alpha and changes quickly. The experiment pins 0.10.0, the
latest evaluated release, rather than placing an old release in production to
preserve a superficial MSRV match. Its `cubecl-zspace` 0.10.0 dependency
declares Rust 1.92. More importantly, the common CUDA/HIP C++ type registry in
0.10.0 still comments out scalar `f64`. Eqiora checks `Buffer` and
`Arithmetic` use before allocating. A physical NVIDIA test observes the
expected typed rejection.

There is no available physical AMD runner, and the shared missing `f64`
capability already prevents a conforming HIP launch. ROCm value comparison,
ordered/fast output comparison, generated-cache identity, and performance are
therefore pending rather than reported as passes.

## Graduation gate

CubeCL can move into a production L3 adapter only when all of the following
hold for a supported release:

1. CUDA and HIP advertise the required `f64` buffer and arithmetic behavior.
2. Production MSRV policy is compatible without patching upstream source.
3. CPU, physical CUDA, and physical ROCm values pass the same local-action
   cases in one through three dimensions.
4. Ordered and fast policies have distinct evidence and tolerances.
5. Generated code/cache identity, compiler options, and runtime/device version
   are inspectable.
6. Compilation latency, binary size, and transfer-inclusive performance are
   measured against a relevant vendor path.

Until then, CubeCL is neither a support claim nor a dependency of a stable
Eqiora crate.

## Alternatives considered

### Put CubeCL arrays in Operator IR

This would make one compiler/runtime version part of the lowered mathematical
contract. Rejected.

### Use CubeCL 0.6 to retain Rust 1.85

That release builds on the production MSRV but also disables scalar `f64` for
the CUDA C++ backend. It cannot meet the numerical contract and would evaluate
an obsolete API. Rejected.

### Convert the `f64` action to `f32`

This would silently change the admitted scalar representation to make a
backend pass. Rejected. Mixed precision requires its own Realization policy,
error contract, and evidence.

### Generalize immediately to ragged cells and global scatter atomics

That combines local operator shape, global ownership, and reduction policy
before one falsifying local case exists. Deferred behind separate contracts.

## Compatibility and safety

The production change adds only an owned L2 IR and one L3 numerical lowerer;
no wire or Semantic Model format changes. The experiment is unpublished and
excluded from the root workspace. Its small unsafe boundary constructs
CubeCL array arguments only after Eqiora has proven exact buffer lengths. Raw
handles, source strings, compiled modules, and device pointers are not public
inputs.

## Verification

1. Reject zero, ragged, non-finite, or shape-overflowing local-action IR.
2. Reject wrong or non-finite packed inputs before a backend call.
3. Match assembled CSR and the registered global packet action in 1D, 2D, and
   3D, including constrained RHS, transpose, diagonal, and accepted CG values.
4. Compile and test the isolated CubeCL adapter without a device.
5. On a physical CUDA device, reject before allocation when CubeCL omits
   required `f64` capability.
6. Keep physical ROCm and value/performance evidence open until the capability
   and hardware gates can genuinely run.

## Research basis

- [CubeCL 0.10.0 documentation](https://docs.rs/crate/cubecl/0.10.0)
  describes the project as alpha and lists CUDA and HIP among its backends.
- [CubeCL 0.10.0 C++ type registration](https://github.com/tracel-ai/cubecl/blob/v0.10.0/crates/cubecl-cpp/src/shared/base.rs)
  is the common backend capability source and omits scalar `f64`.
- [`cubecl-zspace` 0.10.0 package metadata](https://docs.rs/crate/cubecl-zspace/0.10.0/source/Cargo.toml)
  declares Rust 1.92.

The upstream library owns kernel compilation and runtime mechanics. Eqiora
owns admission, stable local-action meaning, numerical policy, and evidence.
