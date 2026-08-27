# Installed-Python private fixed-reference FSI gallery build

This case verifies one private publication adapter over the already accepted
two-state fixed-reference FSI trajectory. A checked-in ordinary Python script
imports an installed `eqiora` wheel, authors the adjacent Geometry and common
Mesh, compiles the packaged component source, resolves the exact Model with
scoped MINI/P1 and P1 policies, and consumes the common `Plan`, `State`, and
`Result`. Its typed FSI evidence is keyed by exact `State`. The script renders
a lossless poster, VP9 WebM, H.264 MP4,
a distinct two-panel reduced-motion still, and a descriptive text alternative,
then records complete source, lineage, scene, encoder, environment, and output
identity in canonical private JSON.

The media remain visibly marked as a development preview. Their record retains
the truthful scientific and trajectory evidence IDs but cannot pass production
admission: its publication status and experience ID are deliberately outside
the contracted production gallery. A synthetic in-memory candidate exercises
the positive admission path; one-field mutants prove that lineage, evidence,
claim, digest, scene, encoder, environment, and accessibility drift fail closed.

Fluid pressure is the film's sole primary field. Solid displacement is shown
only as an explicitly exaggerated geometry state. Presentation interpolation
between the two accepted states is permanently labelled as not solved
dynamics, and the return to the poster changes opacity only, never reversing
the physics. The renderer does not construct a new observable or recreate
materials or boundary conditions that the result does not expose.

Scientific values, tolerances, and acceptance remain owned by
[`fsi.fixed-reference-monolithic-step-2d`](../../fsi/fixed-reference-monolithic-step-2d/README.md)
and
[`artifacts.fixed-reference-fsi-spatial-trajectory`](../../artifacts/fixed-reference-fsi-spatial-trajectory/README.md).
The installed Python common-Result and typed-evidence boundary remains owned by
[`interfaces.python-fixed-reference-fsi-demo`](../python-fixed-reference-fsi-demo/README.md).
The complete executable claim and non-claims are frozen in
[`case.toml`](case.toml).

Run:

```bash
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-fixed-reference-fsi-gallery-build
```
