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

### 3. Residuals — per route, and the true and weak bounds are different formulas

Residuals are not cross-route observations, and this gate does not treat them as
any. Each route is required, on its own, to satisfy the frozen contract. The
contract states **two different bounds**, and this gate keeps them apart.

**True reduced residual** — that route's own selected target plus that route's
own **recorded** roundoff allowance, the contract's
`4096 * eps * (1 + ||A||_inf * ||x||_inf + ||b||_inf)`, which each route
evaluated in its own working precision and published. The gate reads it and does
not recompute it.

```text
true_reduced <= own_selected_target + own_recorded_roundoff_allowance
```

**Weak pressure-row residual** — the same selected target plus *the existing
pressure-row roundoff allowance*, which is a different formula, owned by the
production weak-incompressibility acceptance check rather than by this package:

```text
roundoff  = 4096 * eps * (1 + norm + |multiplier| * ||gauge||_2 + target)
tolerance = target + |multiplier| * ||gauge||_2 + roundoff
```

Both routes publish a `BoundaryTraction` pressure reference with **no gauge row,
no gauge column and no gauge multiplier** — asserted structurally in the same
run — so every gauge term is exactly zero and drops out. The bound the gate
applies is therefore

```text
weak_roundoff = 4096 * eps * (1 + weak_norm + selected_target)
weak_limit    = selected_target + weak_roundoff
```

recomputed here in binary64, per weak norm, in that association order. Adding an
exactly-zero gauge term to a positive finite binary64 value is exact, so the
dropped terms change no bit — but this is the formula transcribed and evaluated
here, not a production run: the existing acceptance function was **read, never
executed**, and there is still no production implementation of this capability.

The recorded true-residual allowance is **not** the pressure-row allowance and
is not used for a weak residual. On this witness it is `6.48e-05`, about
`489` times looser than the `1.32e-07` the contract actually names. The frozen
weak norms pass the correct, tighter bound by an enormous margin, so no value in
this package changes — but the gate now measures the residual it says it
measures. Before the witness-tuple amendment the same two numbers were
`6.47e-05` and `2.23e-12`, a separation of `2.9e+07`; the pressure-row bound
tracks the selected target and the true-residual allowance does not, so amending
the target narrowed the gap between them.

| route | quantity | value | bound | limit | margin |
| --- | --- | --- | --- | --- | ---: |
| python | true reduced | `8.193525e-38` | recorded allowance | `6.482749e-05` | `7.91e+32` |
| python | weak pressure row (2-norm) | `3.142092e-40` | pressure-row allowance | `1.323972e-07` | `4.21e+32` |
| julia | true reduced | `5.421072e-73` | recorded allowance | `6.482749e-05` | `1.20e+68` |
| julia | weak pressure row (2-norm) | `1.621881e-75` | pressure-row allowance | `1.323972e-07` | `8.16e+67` |
| julia | weak pressure row (inf-norm) | `8.162678e-76` | pressure-row allowance | `1.323972e-07` | `1.62e+68` |

Both limits are derived from the selected residual target, so the amendment of
the frozen relative tolerance from `1e-11` to `1e-6` moved them mechanically:
the true-residual limit by `0.2 %`, and the **weak pressure-row limit by five
decades**, from `2.233457e-12` to `1.323972e-07`. No route value changed and no
verdict changed — the frozen weak norms clear both bounds by more than 32
orders of magnitude — but the gate's power to reject a mutated weak residual is
correspondingly coarser, and the mutation record below says which probes that
affects. See [`../amendment/README.md`](../amendment/README.md).

The **2-norm is the contractual weak norm**. The Julia route also publishes an
inf-norm; it is bounded under the same formula so that a published value stays
audited rather than merely printed, but it is a diagnostic and not a second
independent criterion — passing it is implied by the 2-norm result, because
`||r||_inf <= ||r||_2`. The two routes' near-zero weak values are never compared
with each other.

That allowance depends on the norm it bounds, exactly as the production check
does, so the acceptance region is the fixed point of
`n <= target + 4096 * eps * (1 + n + target)`. The fixed point is evaluated from
the selected target each run, so the amendment of that target moved it; the
value the gate now records is in the table above.

**Domain before magnitude.** A norm cannot be negative and neither can a
roundoff allowance, so every bounded quantity, every selected target, every
allowance and every resulting limit must be finite **and nonnegative** before
any magnitude is compared. Each route's selected target and recorded
true-residual allowance are checked explicitly, and no bound is formed for a
route that fails them. A `value <= limit` test alone would accept a negative
residual norm outright and would accept a negative allowance that silently
shrinks the bound.

The two routes solve at different working precisions, so neither their residuals
nor their allowances are compared with each other. The quantities the two routes
genuinely share are still required to be **bit-identical**, and are: the
selected residual target, the operator infinity norm, the solution infinity norm
and the reduced right-hand-side 2-norm. The shared selected target is
additionally validated per route, so making it negative in *both* documents
cannot pass on cross-route equality alone.

#### The routes' own weak self-checks used the looser reading

Each route's frozen document states its weak pressure-row residual against the
same allowance it recorded for its true residual, and each route's own checker
applied that reading. Those documents are **not** edited: they are their
authors', and no number in them changes. What changes is which bound the meeting
gate applies to those already-frozen values — the existing pressure-row formula
rather than the borrowed true-residual one. This is an interpretation
correction at package level, not a route-value change, and both frozen weak
norms pass the corrected, `2.9e+07` times tighter bound by more than `27` orders
of magnitude.

## What is compared

`275` checks — `231` structural, `39` numeric, `5` bounded — all passing.

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
- the shared residual target, operator, right-hand-side and solution scales;
  each route's selected target and recorded true-residual allowance required
  finite and nonnegative before any bound is formed; and each route's residual
  bounds — the true reduced residual against its own recorded allowance, and
  every weak pressure-row norm against the recomputed pressure-row allowance;
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
route's true residual pushed one ulp past its own target-plus-allowance bound.
Those weak pressure-row mutations were measured against the looser bound the
gate applied at the time; the residual set below is the current one and
supersedes them.

Eight further mutations exercised the digest guards: appending a byte to either
route document, to the shared mesh, or to the gate's own source is rejected both
by `--check`, which refuses to rewrite a report whose recorded digests moved,
and by the packaging-fidelity package check below.

### Residual bounds, their domain, and that they still admit

A cross-provider review found that testing only `value <= limit` accepted a
negative residual norm and a negative recorded allowance, and that the weak
pressure-row residuals were being measured against the true-residual allowance
rather than the existing pressure-row one. Thirty-five further mutations were
run after that correction, each rejected with a non-zero exit:

- `-1.0` substituted for **each of the five bounded residual records** — both
  routes' true reduced residuals, both weak 2-norms and the Julia weak inf-norm;
- a **negative selected target** in the Python document, in the Julia document,
  and in **both together**, which preserves cross-route equality and is still
  rejected by the per-route domain guard;
- a **negative recorded true-residual allowance** in each route;
- a weak 2-norm at `2.2334574668971316e-12`, **one ulp above the weak bound as
  it then stood**, in each route — and the same for the Julia inf-norm;
- a weak 2-norm at `1e-11` and at `6.4e-05` in each route, both of which the
  reading before that correction accepted;
- each route's true reduced residual one ulp past its own recorded bound;
- `NaN` and `+inf` residuals;
- and the pre-existing guards, re-confirmed intact against this same build: a
  probe vertex moved one ulp off the shared mesh, a barycentre replaced by
  another cell's, reordered tie candidates, an introduced gauge row, a pressure
  reference changed to `ZeroIntegral`, a value pushed past tolerance in each of
  the four physical families, a relabelled route, a changed mesh facet count,
  and a byte appended to each of the two route documents and the shared mesh.

Four further probes had to be **accepted**, so that none of the above is
consistent with a bound that simply rejects everything: each route's weak 2-norm
set to `2.233457466897131e-12`, the largest binary64 value the existing weak
bound admits; the Python true residual set exactly to its own bound; and a weak
norm of `+0.0`.

Those mutations were applied to throwaway copies outside the repository and are
not part of the frozen tree.

**Three of them no longer reject, and the amendment is why.** The weak
pressure-row bound is derived from the selected residual target, which the
witness-tuple amendment moved from `1.3239627651209673e-12` to
`1.3239627651209673e-07`; the bound moved with it, from `2.233457e-12` to
`1.323972e-07`. The weak 2-norm probes at `2.2334574668971316e-12` and at
`1e-11`, and the corresponding Julia inf-norm probe, now fall inside the bound
and would be accepted. The `6.4e-05` probe still rejects. This mutation set was
run against the pre-amendment bound and is left as the record of that run
rather than rewritten; the loss of discrimination is stated in
[`../amendment/README.md`](../amendment/README.md) rather than absorbed
silently. Every other mutation listed above is unaffected, because no other
bound in this gate depends on the selected target.

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
