# Release notes

Eqiora `0.1.0a7` is the current public alpha. APIs and saved-file formats may
change before 1.0; review the changes below when upgrading.

## 0.1.0a7

Scalar conservation supports Cartesian problems in 1D–3D. Continuum models
share material and kinematic definitions across elasticity, transient flow,
and fluid–structure interaction.

Runs now reuse prepared structure and Faer factorizations. Python `Source`
adds natural equation authoring, model-local aliases infer dimensions, and
cancellation exposes the last accepted State. Mesh generation uses `MeshPlan`.

Material compositions can combine multiple property releases in a Component
Law from Eqiora source, Python, or a package. `Eqiora.Solid@0.2.0` provides
a composition for Young's modulus and Poisson's ratio.

When upgrading, replace removed legacy dimension aliases with the current
names. Colab is the maintained hosted-notebook example; duplicate Marimo and
Jupyter examples have been removed.

## 0.1.0a6

The language adds scalar property bindings, affine coefficients, scalar primal
formulations, directional Stokes correspondence, `math.pi`, typed model-local
`let`, and derived-dimension aliases. Equivalent additive equation orientations
compile to the same residual.

Plans, Results, and Trajectories can be saved and reopened as local files.
Rust adds a common transient `RunRequest`. Cylinder meshes and startup media
have been refined for clearer presentation.

## 0.1.0a5

Resolved Plans, restartable States, Trajectories, and Results can be serialized
and reopened from Python. Reloading checks that the model and other required
inputs match.

Static scalar, elasticity, and Stokes runs return a Result directly. The
optional `eqiora.View` displays 2D geometry, meshes, selections, and scalar
fields in Python notebooks.

## 0.1.0a4

Python can compile Eqiora equations with geometry, choose numerical settings,
and run steady and transient flow. The release adds a GitHub-backed Colab
walkthrough of the first ten cylinder-flow startup steps. Its short run
illustrates startup rather than a developed wake or benchmark comparison.

The Studio packaged DC-drive example remains executable in this release.

## 0.1.0a3

The exact-cylinder Python example uses Gmsh 4.15.2 to generate its mesh and
plot the resulting pressure field.

## 0.1.0a1

See the [original release](https://github.com/nkiyohara/eqiora/releases/tag/v0.1.0a1)
for its source, package artifacts, and supported platforms.

The repository [changelog](https://github.com/nkiyohara/eqiora/blob/main/CHANGELOG.md)
records additions, fixes, and migrations. Browse the
[capability matrix](capabilities.md) for available functionality and the
[evidence guide](evidence/index.md) for numerical checks.
