# Expected structural evidence

The ordinary positive described in the parent README must pass before these
mutants are evaluated. Exact zero is only a linked predecessor regression.

For affine MINI basis functions `psi_a`, components `r,c`, and positive
constant `mu`, the independent direct oracle fixes

```text
G_ab = integral grad(psi_a).grad(psi_b)
X_(a,r;b,c) = integral partial_c(psi_a) partial_r(psi_b)

K_DFG_(a,r;b,c) = mu delta_(r,c) G_ab
K_SYM_(a,r;b,c) = mu [delta_(r,c) G_ab + X_(a,r;b,c)]
K_DFG = K_SYM - mu X
```

The same single crossed subtraction applies to the accepted residual, every
line-search residual, the analytic Jacobian, and the centered-Jacobian audit.
Mass, energy-skew convection, pressure, continuity, source, traction,
ordering, and essential elimination remain unchanged.

The registered positive scopes a test-only probe around the actual
source-bound advance. For every observed local pair it independently computes
`mu delta_(r,c) grad(psi_a).grad(psi_b)`, checks the returned production pair,
observes a P1 tuple with nonzero crossed action, and observes a nonzero
`a=b=4` MINI-bubble action. On the P1 discriminator it explicitly distinguishes
no subtraction (`direct+crossed`), one subtraction (`direct`), two
subtractions (`direct-crossed`), and sign-reversed subtraction
(`direct+2 crossed`). A second run poisons the probed return; accepted advance
would prove the helper was unused and must fail the test.

On the unit reference triangle, the exact discriminators are:

- `u=(x,x)`, `v=(x+y,0)`: DFG/crossed/symmetric are `1/2, 1, 3/2`
  for `mu=1`;
- trial `lambda_2 e_x`, test `lambda_2 e_x`: `1/2, 1/2, 1`;
- trial `lambda_3 e_x`, test `lambda_2 e_y`: `0, 1/2, 1/2`;
- for `b_beta=beta lambda_1 lambda_2 lambda_3`, DFG/symmetric diagonal
  bubble entries are `mu beta^2/90` and `mu beta^2/60`, while the selected
  off-component entries are `0` and `mu beta^2/360`; and
- the pressure and continuity moments with `p=q=lambda_1` and
  `v=u=lambda_2 e_x` are both `-1/6`.

“Nonsymmetric” describes the DFG stress carrier, not the pure viscous matrix.
The direct DFG bilinear is symmetric positive semidefinite. A test demanding a
nonsymmetric pure viscous matrix is wrong.

## Sixteen mutant dispositions

1. Symmetric `2 mu eps(u)` volume is rejected by P1 and bubble direct-versus-
   crossed identities.
2. Omitted, sign-reversed, or twice-subtracted crossed action is rejected by
   the exact off-component count and primal/Jacobian consistency.
3. Omitted, doubled, transposed, or foreign viscosity is rejected by one
   sealed viscosity identity and nonzero diagonal/off-component actions.
4. DFG volume with symmetric outlet, or its converse, fails the single stress
   identity gate before assembly.
5. Equal zero facet bytes cannot pass without prior DFG outlet proof.
6. Reversed parent normal fails the exact `u=-s(-1,0)=(s,0)` inlet sign.
7. `Umean` in place of `Umax` fails centre `Umax` and mean `2 Umax/3`.
8. Coordinate classification fails correspondence-owned vertex assignment
   and shared-vertex reconciliation before assembly.
9. Missing, renamed, overlapping, or incomplete sets reuse the accepted
   exact five-set partition rejection.
10. Foreign source/mesh owner or stale Model/revision/state/Realization reuses
    exact identity admission and proves DFG choice is sealed into that owner.
11. A gauge row or free pressure shift fails because `p+c` changes outlet
    traction by `-c n`; full coefficient width is `3V+2C`, not `+1`.
12. Pressure-sign reversal changes the exact local moments from `-1/6` to
    `+1/6`.
13. Flux/normal/divergence/facet-audit drift reuses the accepted skew evidence
    and exact `C_cons-C_skew=(B+D)/2` parent-normal identity.
14. An inadmissible nonzero initial state reuses fail-before-Newton essential
    trace and weak-continuity admission.
15. A public/runtime stress selector fails the structural surface check; only
    the exact private source binding selects DFG.
16. An exact-zero-only package fails because the nonzero source-bound checked
    step is the first obligation.

The exact finite structural witness freezes

```text
(V,C,B,Qc,Qf,S,N,L,K) = (13,17,9,25,2,1,16,12,2000)
unknowns = 3V+2C = 73
A = 2*unknowns = 146
sparse_nnz = unknowns^2 = 5329
```

Here `Qc=5*5` and `Qf=2` are the already selected quadrature point counts;
`A` is the implementation-independent columnwise centered-audit upper bound;
and `sparse_nnz` is the dense structural upper bound. Packet count is
`C+B_outlet=19 <= C+B=26`. The exact abstract-work construction is

```text
S * (N*(L+1)+A+1) * (C*Qc+B*Qf+K*sparse_nnz)
= 1 * (16*13+146+1) * (17*25+9*2+2000*5329)
= 3,783,747,265.
```

The exact oracle performs checked construction of every addition and
multiplication and supplies targeted overflow witnesses for `3V`, `2C`, their
sum, `C+B_outlet`, `L+1`, `N*(L+1)`, `A+1`, their sum, `C*Qc`, `B*Qf`,
`K*sparse_nnz`, both spatial sums, and the outer `S` product. These are
deterministic abstract-operation and coefficient bounds, not reduced-width, allocation,
residency, storage, wall-clock, production-scale, GPU, MPI, or performance
claims. No numerical acceptance tolerance or benchmark expected value exists
in this directory.
