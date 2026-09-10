# Fixed-domain conservation Laws

A `law` retains physical flux and source as mathematical terms. Numerical methods
consume these terms through the same Model as ordinary Relations.

```eqiora
model HeatedInterval() {
  domain body = box(0, 1);
  domain left = boundary(body, axis = 0, side = lower);
  domain right = boundary(body, axis = 0, side = upper);
  variable temperature: K on body;
  parameter conductivity: kg * m / s^3 / K = 2;
  parameter heating: kg / m / s^3 = 4;
  law heat_balance on body {
    flux -conductivity * grad(temperature);
    source heating;
  }
  relation left_temperature on left { trace(temperature) = 0; }
  relation right_temperature on right { trace(temperature) = 0; }
}
```

A Law specifies the steady balance `div(flux) = source`. Flux is the
physical outward flux; diffusion therefore uses `-conductivity * grad(temperature)`.
The compiler requires exactly one flux and one source, with compatible dimensions
and the Law's exact volume support. Write `source 0;` for no production.
Boundary and interface conditions remain ordinary Relations on their exact supports.

Python Module authoring uses `component.law("heat_balance", on=body,
flux=-conductivity * eqiora.lang.grad(temperature), source=heating)`. It produces the
same source AST and retains the same physical terms in Model artifacts and package
composition. Changing a physical term changes Model meaning even when an equation
could otherwise be rewritten to an equivalent residual.

The admitted boundary is real scalar steady conservation on a fixed volume. Existing
scalar diffusion realizations impose their own coefficient, Geometry, boundary and
method restrictions. The parser rejects storage terms; no stored quantity is admitted
until accumulation correspondence has a checked implementation. This does not implement
transient thermal execution, moving-domain transport,
arbitrary vector Laws, or general authored Law-to-form correspondence.
