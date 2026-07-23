# Reference contract

- [RFC 0043](../../../../rfcs/0043-simplicial-mini-stokes-realization.md)
  owns the exact numerical claim and nonclaims.
- Arnold, Brezzi, and Fortin,
  [*A stable finite element for the Stokes equations*](https://www-users.cse.umn.edu/~arnold/papers/stokes.pdf),
  proves the stability basis for enriching continuous piecewise-linear
  velocity with simplex bubbles while retaining continuous piecewise-linear
  pressure.
- Duffy,
  [*Quadrature over a pyramid or cube of integrands with a singularity at a vertex*](https://doi.org/10.1137/0719090),
  gives the coordinate-transform family used here to map positive tensor
  Gauss--Legendre quadrature to a simplex. Eqiora makes only the narrower
  total-degree-four assembly and total-degree-six error-rule claims exercised
  by its own monomial tests.
- Paige and Saunders,
  [*Solution of sparse indefinite systems of linear equations*](https://doi.org/10.1137/0712047),
  is the original MINRES reference. Eqiora independently verifies the true
  residual of its bounded reference implementation.

## Independent manufactured oracle

Let

```text
u = (x^2, -2xy),
p = x - 1/2,
mu = 1.
```

Then

```text
grad(u) = [[2x, 0], [-2y, -2x]],
div(u)  = 2x - 2x = 0,
```

and

```text
2 sym(grad(u)) - p I
  = [[3x + 1/2, -2y],
     [-2y, -5x + 1/2]].
```

Its row-wise divergence is `(1, 0)`, hence

```text
-div(2 sym(grad(u)) - p I) = (-1, 0) = f.
```

Moreover,

```text
integral_0^1 integral_0^1 (x - 1/2) dy dx = 0,
```

so no post-hoc pressure shift is needed when evaluating the exact error.
The divergence, pressure mean, and forcing identities are derived here rather
than copied from an external solution table.

## Quadrature-degree rationale

On an affine triangle, the normalized MINI bubble is cubic in barycentric
coordinates and its reference gradient is quadratic. The viscous bubble--
bubble product therefore has total polynomial degree four. P1-pressure times
bubble-gradient coupling has degree three, the constant-force bubble load has
degree three, and the gauge term has degree one. Exactness through total degree
four is consequently sufficient for every local term in this manufactured
case and is the minimum admitted assembly contract.

The discrete MINI velocity is cubic while the manufactured exact velocity is
quadratic, so the squared velocity error can have total degree six. Error norms
therefore use a separate positive four-point-per-axis Duffy rule declared exact
through total degree six. This stricter oracle rule is not an additional
assembly requirement.

The citations motivate the method, mapping, and iteration. Acceptance still
depends on Eqiora's executable structural, residual, convergence, gauge,
balance, and falsifier checks; the papers are not treated as run evidence.
