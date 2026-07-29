# Dual independent oracle gate

Two agents derived the expected values for this slice separately from the public
claim, each without reading the other's route, any production implementation, or
any existing `verify/fluid` case. This directory is where those two frozen
results meet.

`compare_routes.py` reads **only** the two packaged frozen JSON documents and
the packaged shared mesh. It assembles nothing and solves nothing. Its result is
frozen in [`expected/agreement-report.json`](expected/agreement-report.json).

```bash
python3 verify/fluid/exact-circular-hole-stokes-2d/agreement/compare_routes.py
python3 verify/fluid/exact-circular-hole-stokes-2d/agreement/compare_routes.py --check
```

Exit status `0` is PASS. Anything else is RETURN.

The report is byte-reproducible: `--check` regenerates it and fails rather than
rewriting if it would differ. It was confirmed to reproduce identically under
three distinct `PYTHONHASHSEED` values, so no interpreter-dependent iteration
order leaks into the frozen record.

## The tolerance formula is an input, not a knob

Every physical comparison uses exactly

```text
abs(a - b) <= absolute_floor + 2e-10 * physical_scale
```

| family | floor | physical scale | tolerance | measured max difference | margin |
| --- | --- | --- | --- | --- | ---: |
| velocity `m/s` | `2e-12` | `0.3` | `6.2e-11` | `1.084202e-19` | `5.72e+08` |
| pressure `Pa` | `2e-14` | `0.0007317073170731707` | `1.663415e-13` | `0` | exact |
| signed flux `m^2/s` | `2e-13` | `0.123` | `2.48e-11` | `1.721916e-41` | `1.44e+30` |
| reaction / balance `N/m` | `2e-14` | `0.0003` | `8e-14` | `2.775558e-17` | `2.88e+03` |

Two comparisons fall outside those four families, and the report labels both as
such rather than folding them in:

- **Geometric selector coordinates** (probe targets, cell barycentres, probe
  vertices, tie-candidate vertices) are compared under the velocity-probe
  tolerance — the family whose selection they determine. Measured maximum
  `1.110223e-16 m` against `6.2e-11 m`.
- **Dimensionless solver diagnostics** (here only the roundoff allowance, which
  the two routes evaluate at different working precisions) are compared as
  binary64 representation agreement at `<= 4` ulp. Measured maximum `1` ulp. The
  selected residual target, operator infinity norm, solution infinity norm and
  reduced right-hand-side 2-norm are required to be **bit-identical**, and are.

Neither addition relaxes anything: both measured maxima are reported beside
their limits so a reader can apply a stricter reading without rerunning.

## What is compared

`291` checks — `213` structural, `77` numeric, `1` diagnostic — all passing.

- all 5 velocity probes: both components, plus target and cell-barycentre
  selectors and the tied-cell count;
- all 6 pressure probes: value, vertex selector, exact tie count, **both**
  two-way tie candidate sets in full, the lexicographic ordering of the
  candidate list, and that each route selected the lexicographic minimum;
- signed inlet, outlet and sum flux, with inlet negative and outlet positive
  under the parent-outward normal;
- both labelled cylinder reaction orientations, and that each route publishes
  them as exact componentwise negations, with the fluid force on the cylinder
  along `+x`;
- global balance: constrained reaction, integrated body force, integrated
  applied traction, and the componentwise sum;
- mesh counts, the complete boundary partition `14 / 2 / 38 / 50` covering all
  104 facets exactly once, the frozen quad diagonal `O_i--I_j` and the ordered
  cell pair `(O_i,O_j,I_j)`, `(O_i,I_j,I_i)`;
- the scale profile `L`, `U`, `P`, `G`, `Theta`, the exact-decimal `Theta`
  spelling, `mu` and `mu_hat`;
- DOF counts `208 / 208 / 104 / 520 / 206 / 314`, zero gauge rows, 103 essential
  and 1 free velocity vertex, and cell-interior bubbles;
- residual target, roundoff allowance, operator, right-hand-side and solution
  scales;
- the `BoundaryTraction` pressure reference on both sides, and the absence of
  any gauge row, gauge column, gauge multiplier or `ZeroIntegral` constraint.

## What makes it fail

The gate is fail-closed by construction: unit-bearing container key sets, probe
inventories and probe orders are pinned, the two routes' distinct probe label
vocabularies are pinned separately so a third spelling cannot be absorbed by
positional matching, and every compared value is required to be finite.

Twenty-one mutations of the frozen inputs were each rejected with a non-zero
exit: a value pushed past tolerance in each of the four families; a missing
probe; an extra probe; reordered probes; a relabelled probe; a relabelled
route; a broken reaction negation; a renamed velocity unit key; a renamed flux
unit key; a non-finite value; an introduced gauge row; a changed pressure
reference; a changed mesh facet count; reordered tie candidates; a changed DOF
count; a changed scale; a broken mesh partition; and a changed quad diagonal.
Those mutations were applied to throwaway copies outside the repository and are
not part of the frozen tree.

## What agreement authorizes

Agreement of the two independently derived routes **authorizes implementation
against the frozen contract**. It does **not** verify production: no production
implementation of this capability exists, none was read, and none was executed
here. The routes' own frozen documents each still say the gate had not passed —
that is each route's true statement about itself in isolation, made before this
comparison existed, and it is superseded only by
[`expected/agreement-report.json`](expected/agreement-report.json).

Read the case [`README.md`](../README.md) before acting on this result: the
coarse-mesh non-claims and the Julia route's measured advisory about the frozen
solver selection both bound what may be built on it.

## Packaging fidelity

`check_packaging_fidelity.py` is not part of this gate. It is the argument that
packaging changed prose only — see [`../README.md`](../README.md).
