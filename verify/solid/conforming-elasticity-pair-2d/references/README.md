# Reference contract

- [RFC 0041](../../../../rfcs/0041-complete-exterior-port-families.md)
  defines the package-neutral elasticity boundary inventory.
- [RFC 0042](../../../../rfcs/0042-conforming-elasticity-interface-realization.md)
  defines the exact two-body topological quotient and finalized evidence.
- [RFC 0039](../../../../rfcs/0039-canonical-isotropic-elasticity-2d.md)
  defines the admitted canonical isotropic-elasticity subset.
- [RFC 0035](../../../../rfcs/0035-field-valued-boundary-interfaces.md)
  defines field-valued trace/flux meaning and parent-outward orientation.
- The immutable `Eqiora.Solid.LinearElasticity@0.3.0` source authority is
  [`mixed-boundary-elasticity-2d/package-v0.3.0`](../../mixed-boundary-elasticity-2d/package-v0.3.0/README.md).

## Independent heterogeneous oracle

Let the unit square be split at `a = 1/2`. Take `ell = 1`, `lambda = 0`,
`mu_L = 3`, `mu_R = 6`, and

```text
q_L = 2 mu_L x / ell = 6x,
q_R = mu_R x / ell   = 6x.
```

The exact displacement is

```text
u_L = (x - x^2 / 2, 0),                         0 <= x <= 1/2,
u_R = (3/16 + x/2 - x^2/4, 0),                1/2 <= x <= 1.
```

Both traces equal `[3/8, 0]` at the interface. The one nonzero strain component
is `1 - x` on the left and `1/2 - x/2` on the right, so its two interface
limits are `1/2` and `1/4`. Nevertheless,

```text
sigma_L,xx(1/2) = 2 mu_L (1/2) = 3,
sigma_R,xx(1/2) = 2 mu_R (1/4) = 3.
```

With parent-outward normals `+e_x` and `-e_x`, the exact interface tractions are
therefore `[3, 0]` and `[-3, 0]`. Since `grad(q) = [6, 0]`, each half-body has
integrated load `[3, 0]`. The only external reaction is `[-6, 0]` on `x = 0`.

## Exact Q1 interpolation errors

Give each half-domain `n` cells in each Cartesian direction. Its horizontal
cell width is `h = 1/(2n)`; the vertical width does not affect this
one-dimensional displacement oracle. For a quadratic with second derivative
`-1`, the squared interpolation errors over a horizontal cell are `h^5/120` in
L2 and `h^3/12` in H1 seminorm. The right displacement has half that curvature,
so its squared errors are one quarter as large. Summing across the two
half-width domains and the unit height gives

```text
||u - u_h||_L2^2 = h^4/240 + h^4/960 = h^4/192,
|u - u_h|_H1^2   = h^2/24  + h^2/96  = 5h^2/96.
```

## Weak action versus raw recovered traction

The body-local finite-element cut residual represents the weak action exerted
across the omitted interface boundary. Exact integration and the manufactured
nodal solution give the opposite resultants `[3, 0]` and `[-3, 0]` at every
refinement.

By contrast, Q1 strain is constant on each cell. The last left cell samples the
exact linear strain at `x = 1/2 - h/2`, while the first right cell samples it at
`x = 1/2 + h/2`. Applying the two outward normals gives

```text
t_L,h = [ 3 + 3h, 0],
t_R,h = [-3 + 3h, 0],
t_L,h + t_R,h = [6h, 0].
```

This first-order raw recovery error is a separate diagnostic. It does not
contradict the exact algebraic equilibrium of the conforming quotient system.
