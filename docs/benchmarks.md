# Published benchmarks

Which published benchmark problems Eqiora can reproduce, and for the ones it
cannot, the named thing that stops it.

This is not a plan and not a wish list. [`docs/roadmap.md`](roadmap.md) decides
order; this file only records reachability. Every row carries a citation that
`tools/ci/check_docs.py` resolves, so a row cannot survive the capability it
describes being renamed or removed.

## How to read a case manifest

A registered case declares a `[claim_boundary]`. **Those clauses bound that
case, not the implementation.** `fluid.simplicial-mini-stokes-2d` sets
`natural_or_open_boundary = false` because that case poses a complete essential
boundary — not because traction boundaries are missing.
`fluid.mixed-static-pressure-mini-stokes-2d` poses three no-slip walls and one
traction outlet and is verified. Reading a single manifest's non-claims as the
edge of the platform understates it, and this document exists partly because
that mistake was made twice while writing it.

To ask what the platform can express, read the code and the union of the
manifests. To ask what has been *proven*, read one manifest.

## Citations

| Form | Means | Checked by |
|---|---|---|
| `case:<id>` | a registered case, which must exist and be `verified` | the id resolves in `verify/*/*/case.toml` |
| `symbol:<name>` | a named item in the source | the name appears in some `.rs` file |
| `key:<name>` | a declared manifest clause | the key appears in some `case.toml` |
| `none declared` | an observation, not a manifest claim | nothing; the phrase must be literal |

## Reproduced today

Registered, executed in CI, `status = "verified"`.

| Problem | Citation |
|---|---|
| Manufactured Stokes convergence under a non-constant prescribed velocity on the complete boundary | `case:fluid.simplicial-mini-stokes-2d` |
| Mixed Stokes channel: three no-slip walls and one prescribed normal-pressure outlet, pressure fixed by the boundary rather than a gauge | `case:fluid.mixed-static-pressure-mini-stokes-2d` |
| Transient MINI Navier–Stokes on a fixed domain, Newton with an audited analytic Jacobian | `case:fluid.fixed-domain-transient-navier-stokes-2d` |
| Monolithic ALE fluid–structure step, backward Euler, 2D | `case:fsi.fixed-topology-ale-monolithic-2d` |
| The same on tetrahedra, 3D | `case:fsi.fixed-topology-ale-monolithic-3d` |
| Linear isotropic elasticity | `case:solid.isotropic-elasticity-2d` |
| XDMF3 temporal collection plus HDF5 file image, for one 2D remeshing trajectory | `case:artifacts.xdmf-hdf5-remeshing-trajectory` |

## Reachable without new numerical capability

Each needs a mesh, boundary data and a registered case. None needs a new
discretization, boundary vocabulary or solver.

| Problem | Why it is reachable | Citation |
|---|---|---|
| Schäfer–Turek DFG 2D-2 and 2D-3, cylinder in a channel | The inlet is an essential-velocity facet carrying an arbitrary spatial profile, the outlet a constant-traction facet, and the Navier–Stokes assembly already accumulates traction facets alongside essential ones | `symbol:MiniConstantTractionFacet` |
| Backward-facing step | Same boundary pair | `symbol:MiniConstantTractionFacet` |
| Lid-driven cavity, Stokes and Navier–Stokes | A complete essential boundary with a non-zero tangential lid. The lid must be regularized so the trace is continuous at the two upper corners, which is standard for a P1 trace and is a choice of data, not a capability | `case:fluid.simplicial-mini-stokes-2d` |

Reaching these still costs mesh generation, reference quantities such as drag
and lift coefficients or a Strouhal number, and compute time. Reachable means
no new capability is required, not that the work is small.

## Needs new numerical capability

| Problem | What is missing | Citation |
|---|---|---|
| Patch test, Cook's membrane | Elasticity exists only on tensor-product Cartesian Q1. A patch test whose mesh cannot be distorted tests nothing, because arbitrary element geometry is the whole point of it | `symbol:cartesian_elasticity` |
| Turek–Hron FSI2 and FSI3 | The coupled solid is linear. Whether the benchmark's deflection admits a linear solid at all is unsettled here and is stated as unverified rather than assumed | `case:fsi.fixed-topology-ale-monolithic-2d` |
| Any 3D animation | Three-dimensional trajectory export is an explicit non-claim, so the 3D coupled runs compute but cannot be written out | `key:higher_order_or_three_dimensional_export` |
| A general 2D animation of an arbitrary field trajectory | The verified export covers one remeshing trajectory kind, not arbitrary field time series | `case:artifacts.xdmf-hdf5-remeshing-trajectory` |
| Dam break, sloshing, any free surface | No free-surface or interface-capturing formulation exists | `none declared` |
| High-Reynolds flows relying on a turbulence model | No turbulence closure exists; a high-Reynolds run would have to resolve every scale | `none declared` |
