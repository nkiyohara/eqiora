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

A second way to be wrong, which this document was also wrong about: checking
that a benchmark's **boundary conditions** are expressible is not checking that
its **domain** is. The cylinder rows below moved from reachable to blocked for
exactly that reason. Both halves have to be checked, and the domain is the half
that is easy to assume.

A third, found while correcting the second: a capability present in a numerical
object is not the same as a capability a model can reach. The MINI element
genuinely accepts essential and constant-traction facets, and the canonical
realization above it still constructs an all-essential zero-velocity boundary.
Reading the element and stopping there overstates what can be posed.

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

A domain must be a box, because that is the only region the model language can
name. The lid-driven cavity qualifies; a channel around a cylinder does not.

| Problem | Why it is reachable | Citation |
|---|---|---|
| Lid-driven cavity, Stokes and Navier–Stokes | A complete essential boundary with a non-zero tangential lid. The lid must be regularized so the trace is continuous at the two upper corners, which is standard for a P1 trace and is a choice of data, not a capability | `case:fluid.simplicial-mini-stokes-2d` |

Reaching these still costs mesh generation, reference quantities such as drag
and lift coefficients or a Strouhal number, and compute time. Reachable means
no new capability is required, not that the work is small.

## Needs new numerical capability

| Problem | What is missing | Citation |
|---|---|---|
| Schäfer–Turek DFG 2D-2 and 2D-3, backward-facing step, and every other benchmark whose domain is not a box | Two things are missing, and the boundary one was previously understated here. The **domain** cannot be posed. A model may name a `box` and the axis-aligned faces of a box, and nothing else; the CAD surface is box-based; an imported mesh discretizes a region the model already describes rather than supplying one; Gmsh physical groups are refused; and no word for a circle, radius, disk or ellipse exists anywhere in the language, schema, CAD or meshing crates. The **boundary wiring** is also missing on the path that would run it: the constant-traction and non-zero essential facets are real at the MINI element level, but the canonical transient Navier–Stokes realization requires a complete homogeneous velocity trace and constructs an all-essential zero-velocity boundary, so the element's vocabulary does not reach a model | `symbol:CadBoxIntentV1` |
| Patch test, Cook's membrane | Elasticity exists only on tensor-product Cartesian Q1. A patch test whose mesh cannot be distorted tests nothing, because arbitrary element geometry is the whole point of it | `symbol:cartesian_elasticity` |
| Turek–Hron FSI2 and FSI3 | The coupled solid is linear. Whether the benchmark's deflection admits a linear solid at all is unsettled here and is stated as unverified rather than assumed | `case:fsi.fixed-topology-ale-monolithic-2d` |
| Any 3D animation | Three-dimensional trajectory export is an explicit non-claim, so the 3D coupled runs compute but cannot be written out | `key:higher_order_or_three_dimensional_export` |
| A general 2D animation of an arbitrary field trajectory | The verified export covers one remeshing trajectory kind, not arbitrary field time series | `case:artifacts.xdmf-hdf5-remeshing-trajectory` |
| Dam break, sloshing, any free surface | No free-surface or interface-capturing formulation exists | `none declared` |
| High-Reynolds flows relying on a turbulence model | No turbulence closure exists; a high-Reynolds run would have to resolve every scale | `none declared` |
