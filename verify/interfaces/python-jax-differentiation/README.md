# Python JAX typed-FFI differentiation

The optional `eqiora.jax` adapter projects one accepted framework-neutral
`DifferentiableProgram` into JAX without adding a second model or
differentiation semantics. The exact immutable common scalar Plan, its Model,
caller-supplied two-dimensional rectangular Cartesian Mesh, ordered Parameter
identities, and output Field remain static; a traced rank-one array contains
only the numerical Parameter point. The verified Plans use Q1 FEM or
cell-centred TPFA with a typed linear solve policy.

Primal, JVP, and VJP execution use separate typed XLA FFI targets registered
for CPU with API version 1 and lowered by `jax.ffi.ffi_call` custom-call API
version 4. The targets call the same native accepted primal/JVP/VJP actions as
the framework-neutral program. Compiled StableHLO contains Eqiora custom calls,
not `pure_callback` or an XLA Python CPU callback, so neither eager nor `jit`
execution re-enters Python for numerical work.

The adapter accepts only exact static rank-one `float64` arrays on the host CPU
platform and returns the exact static complete-Field shape. The ordinary
verified path uses one unsharded array; its concrete CPU ordinal follows the
input because JAX abstract values do not encode an ordinal. Registration uses
one deterministic key for the complete static program identity and keeps the
native program alive until process exit so a compiled executable can outlive
its temporary Python wrapper. Numerical evaluations and linearizations are not
cached.

The installed-wheel gate uses CPython 3.13 on Linux x86_64 and the exact
JAX/JAXLIB 0.11.0 pair. It exercises:

- Q1 FEM and TPFA FVM primal values against native accepted evaluations;
- eager and `jit` primal, first-order JVP, VJP, and scalar-objective gradient;
- lowered StableHLO containing only the declared typed custom-call path;
- compiled-executable lifetime after the Python program wrapper is released;
- zero tangent and cotangent actions;
- dtype, rank, static shape, CPU platform, direct and explicitly compiled input
  sharding, finiteness, and unknown-program falsifiers;
- explicit rejection of `pmap`, `vmap`, batched derivatives, and higher-order
  linearization, plus immutable static program configuration;
- exact native ABI layout agreement with the installed JAXLIB header; and
- a base `eqiora` import with no JAX or JAXLIB import.

Registered host evidence obtains this profile from the same complete candidate
and manifest used by the base, typing, and PyTorch cases. Its JAX checks must be
present in that accepted manifest; the focused
`tools/ci/python_jax_gate.py` script remains available for standalone
development but is not a second registered artifact build.

This is an exact in-process first-order host-CPU slice. Direct or explicitly
compiled input sharding, `pmap`, `vmap`, and higher-order transformations are
rejected. Explicit output sharding, GPU, TPU, export, serialization,
multiprocessing, and performance are unverified nonclaims. The transformation
seam uses JAX's experimental HiJAX API, so the optional dependency is pinned
exactly rather than presented as a wider compatibility range.

Run the registered installed-wheel evidence with:

```console
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-jax-differentiation
```
