# Reference provenance

The semantic oracle is
[`RFC 0038`](../../../../rfcs/0038-canonical-tensor-structure-operators.md):
`symmetric_part` and `isotropic_lift` retain exact shape, frame, physical
dimension, and volume support in the canonical Relation DAG.

The realization boundary and all numerical or package nonclaims are fixed by
[`RFC 0039`](../../../../rfcs/0039-canonical-isotropic-elasticity-2d.md).

## Local variational oracle

For constant Lamé coefficients, the independent cell bilinear form is

```text
a(u, v) = integral(
  2 mu epsilon(u) : epsilon(v)
  + lambda div(u) div(v)
),

epsilon(u) = (grad(u) + transpose(grad(u))) / 2.
```

Rigid translations and infinitesimal rotation have zero symmetric gradient.
On the unit square, direct integration gives:

```text
u = (y, x)  ->  energy = 2 mu
u = (x, y)  ->  energy = 2 (mu + lambda).
```

The expected values are derived from these expressions, not from the assembled
matrix. The cross-component oracle evaluates the same index-level bilinear
form directly at quadrature points before comparing it with selected matrix
blocks.

## Manufactured solution

On `(0, 1 m) x (0, 1 m)`, define

```text
s_x = sin(k x)
s_y = sin(k y)
q   = q0 (s_x^2 + s_y^2 - 4 s_x^2 s_y^2)
Psi = q0 / (2 k^2 (lambda + 2 mu)) s_x^2 s_y^2
u*  = -grad(Psi)
k   = pi / m.
```

Since

```text
Delta(Psi)
  = q0 / (lambda + 2 mu)
    * (s_x^2 + s_y^2 - 4 s_x^2 s_y^2),
```

we have `q = (lambda + 2 mu) Delta(Psi)`. For
`u* = -grad(Psi)`, the Hessian is symmetric and constant-coefficient isotropic
elasticity gives

```text
div(sigma(u*)) = -(lambda + 2 mu) grad(Delta(Psi)) = -grad(q).
```

Therefore the canonical strong residual
`-div(sigma(u*)) - grad(q)` is identically zero. Both components of `u*`
vanish on the complete boundary because each derivative retains a zero sine
factor on every side.

The integration test evaluates `u*` and its gradient from these closed
expressions independently of the Eqiora spatial tape. No external dataset,
third-party result table, or second executable model supplies the expected
solution.

## Equilibrium oracle

Integrating the strong balance componentwise gives

```text
boundary_reaction + integral(grad(q)) = 0
```

under the registered reaction sign convention. The symmetric manufactured
potential has zero net load, so a second affine potential with known nonzero
constant gradient is used only as a balance falsifier. It changes no material,
space, assembly, or solver contract.

