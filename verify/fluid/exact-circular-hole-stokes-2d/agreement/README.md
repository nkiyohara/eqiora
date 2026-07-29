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

## Three kinds of comparison, and none borrows another's rule

### 1. Physical observations — the precommitted tolerance formula

Velocity, pressure, signed flux and reaction are compared under exactly

```text
abs(a - b) <= absolute_floor + 2e-10 * physical_scale
```

| family | floor | physical scale | tolerance | measured max difference | margin |
| --- | --- | --- | --- | --- | ---: |
| velocity `m/s` | `2e-12` | `0.3` | `6.2e-11` | `1.084202e-19` | `5.72e+08` |
| pressure `Pa` | `2e-14` | `0.0007317073170731707` | `1.663415e-13` | `0` | exact |
| signed flux `m^2/s` | `2e-13` | `0.123` | `2.48e-11` | `1.721916e-41` | `1.44e+30` |
| reaction / balance `N/m` | `2e-14` | `0.0003` | `8e-14` | `2.775558e-17` | `2.88e+03` |

The formula, the floors and the scales are frozen inputs to this gate. An
unsatisfiable oracle is returned with the argument, never relaxed. Nothing else
in this directory is compared under them.

### 2. Geometric selectors — exact, with no tolerance at all

A probe target, a selected cell, a probe vertex and a tie candidate are frozen
mesh geometry in metres, not measurements. They get no tolerance, and none is
invented for them. Every one is reconstructed from
[`../mesh/mesh.json`](../mesh/mesh.json) in `fractions.Fraction` arithmetic over
the parsed binary64 inputs, and required to match exactly:

- all 104 cell barycentres are rebuilt exactly, and the frozen contract's rule —
  minimum squared distance from the target, ties broken by the lexicographically
  sorted vertex-coordinate triple — is evaluated exactly to select the cell and
  the exact tie count for each of the 5 velocity targets;
- each route's reported barycentre is then mapped to the **unique** nearest
  exact mesh-cell barycentre by exact squared distance, and both routes must map
  to the contract-selected cell. Uniqueness is decided by strict inequality on
  exact rationals, not by a margin. The two routes' approximate barycentre
  coordinates are never compared with each other;
- all 6 pressure selectors are rebuilt the same way — the extreme cylinder
  vertex on each axis, then the outer-boundary vertex nearest each named point,
  ties broken by lexicographic coordinate order — and each route's selected
  vertex and complete tie-candidate list must equal the reconstructed set
  bit-for-bit, in order;
- the cell is named by its sorted vertex-coordinate triple, never by an index,
  so the check survives renumbering exactly as the contract requires.

This matters concretely: the two tied cylinder pressure vertices differ by about
`1 Pa`, roughly `6e+12` times the pressure tolerance. Deciding a selector under
a physical tolerance would let exactly the failure this gate exists to catch
pass unnoticed.

### 3. Residuals — per route, against each route's own bound

Residuals are not cross-route observations, and this gate does not treat them as
any. Each route is required, on its own, to satisfy the frozen contract's bound

```text
residual <= own_selected_target + own_roundoff_allowance
```

for both its independently reapplied true reduced residual and its weak
pressure-row residual, using **that route's** selected target and **that
route's** recorded roundoff allowance. The two routes solve at different working
precisions, so neither their residuals nor their allowances are compared with
each other.

The quantities the two routes genuinely share are still required to be
**bit-identical**, and are: the selected residual target, the operator infinity
norm, the solution infinity norm and the reduced right-hand-side 2-norm.

## What is compared

`271` checks — `227` structural, `39` numeric, `5` bounded — all passing.

- all 5 velocity probes: both components, plus the exactly reconstructed target,
  cell and tie count;
- all 6 pressure probes: value, the exactly reconstructed vertex and tie count,
  and **both** two-way tie candidate sets in full, each candidate required to
  equal the reconstructed candidate in the same slot;
- signed inlet, outlet and sum flux, with inlet negative and outlet positive
  under the parent-outward normal;
- both labelled cylinder reaction orientations, and that each route publishes
  them as exact componentwise negations, with the fluid force on the cylinder
  along `+x`;
- global balance: constrained reaction, integrated body force, integrated
  applied traction, and the componentwise sum;
- mesh counts, the complete boundary partition `14 / 2 / 38 / 50` covering all
  104 facets exactly once, the cylinder/outer vertex partition the selector
  reconstruction depends on, the frozen quad diagonal `O_i--I_j` and the ordered
  cell pair `(O_i,O_j,I_j)`, `(O_i,I_j,I_i)`;
- the scale profile `L`, `U`, `P`, `G`, `Theta`, the exact-decimal `Theta`
  spelling, `mu` and `mu_hat`;
- DOF counts `208 / 208 / 104 / 520 / 206 / 314`, zero gauge rows, 103 essential
  and 1 free velocity vertex, and cell-interior bubbles;
- the shared residual target, operator, right-hand-side and solution scales, and
  each route's two residual bounds;
- the `BoundaryTraction` pressure reference on both sides, and the absence of
  any gauge row, gauge column, gauge multiplier or `ZeroIntegral` constraint.

## What makes it fail

The gate is fail-closed by construction: unit-bearing container key sets, probe
inventories and probe orders are pinned, the two routes' distinct probe label
vocabularies are pinned separately so a third spelling cannot be absorbed by
positional matching, and every compared value is required to be finite.

Twenty-eight mutations of the frozen inputs were each rejected with a non-zero
exit: a value pushed past tolerance in each of the four families; a missing
probe; an extra probe; reordered probes; a relabelled probe; a relabelled route;
a broken reaction negation; a renamed velocity unit key; a renamed flux unit
key; a non-finite value; an introduced gauge row; a changed pressure reference;
a changed mesh facet count; reordered tie candidates; a changed DOF count; a
changed scale; a broken mesh partition; a changed quad diagonal; a pressure
probe vertex moved by one ulp off the shared mesh; a tie candidate moved by one
ulp off the shared mesh; a barycentre replaced by another cell's; and each
route's true residual and weak pressure-row residual pushed one ulp past its own
target-plus-allowance bound.

Eight further mutations exercised the digest guards: appending a byte to either
route document, to the shared mesh, or to the gate's own source is rejected both
by `--check`, which refuses to rewrite a report whose recorded digests moved,
and by the packaging-fidelity package check below.

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

`check_packaging_fidelity.py` is not part of this gate, and has two modes.

```bash
python3 verify/fluid/exact-circular-hole-stokes-2d/agreement/check_packaging_fidelity.py
```

With no arguments it is **self-contained**: it recomputes the sha256 of every
document the frozen report says it compared, requires each to equal the digest
recorded there, and walks each frozen JSON rejecting any non-finite numeric
leaf. It reads nothing outside this directory and resolves no git object.

Given source/packaged pairs it instead runs the **historical** prose-only differ
from packaging time — see [`../README.md`](../README.md). Those source files are
not part of this package, no accepted evidence here depends on that mode, and
reproducing this package in future does not require running it.
