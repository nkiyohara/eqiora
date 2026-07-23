# References

- [RFC 0041](../../../../rfcs/0041-complete-exterior-port-families.md)
  specifies the package-neutral downstream boundary inventory and this
  manufactured falsifier.
- [RFC 0039](../../../../rfcs/0039-canonical-isotropic-elasticity-2d.md)
  defines the admitted canonical isotropic-elasticity subset.
- [RFC 0035](../../../../rfcs/0035-field-valued-boundary-interfaces.md)
  defines field-valued physical Connector meaning.

The frozen `package-v0.3.0` directory is the exact source authority for this
case. The live public package must match it byte for byte.

## Independent manufactured oracle

On the unit square with `ell = 1`, `mu = 3`, and `lambda = 0`, let

```text
q = 2 mu x / ell
u = (x - x^2 / (2 ell), 0).
```

Then `grad(q) = (2 mu / ell, 0)`,
`epsilon_xx = 1 - x / ell`, and every other strain component is zero. Thus
`sigma_xx = 2 mu (1 - x / ell)`, `-div(sigma) = grad(q)`, the exact outward
traction vanishes on the right and horizontal sides, and the left resultant
is `(-2 mu, 0) = (-6, 0)`. Integrating the load over the unit square gives
`(2 mu, 0) = (6, 0)`, so the exact force balance closes.

The nodal Q1 field is the piecewise-linear interpolant of the quadratic first
component. On each interval its displacement error is
`t(h - t)/2`, while its derivative error is `h/2 - t`. Summing over the unit
square gives

```text
||u - u_h||_L2^2       = h^4 / 120,
|u - u_h|_H1^2         = h^2 / 12.
```

The Q1 stress is constant in each vertical cell strip. Direct facet
quadrature therefore gives the raw recovered resultants

```text
x = 0:  (-2 mu + mu h, 0) = (-6 + 3h, 0),
x = 1:  (mu h, 0)         = (3h, 0),
y = 0,1:                  (0, 0).
```

The right value converges to the prescribed zero traction at first order.
The left recovered stress resultant is not the constrained algebraic reaction
`(-2 mu, 0)`; the verification keeps those two evidence kinds separate.
