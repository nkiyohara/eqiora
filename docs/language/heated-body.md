# Specimen: a fixed-domain heated body

This [target-language](core.md) specimen specifies a scalar heat Law, complete essential
boundary conditions, initialization, an energy observation, and a weak form. Current continuum
execution does not establish support for this whole source surface; the Law, form, and interface
language slices must land before this becomes an executable example.

## Complete mathematical source

```eqiora
component HeatedBody(
  support body: volume(ambient_dimension = 3),
  support surface: complete_exterior(parent = body),
  parameter capacity: J / (m^3 * K),
  parameter conductivity: W / (m * K),
  parameter heating: W / m^3,
  parameter reference_temperature: K
) {
  state temperature: K on body;

  initial {
    temperature = reference_temperature;
  }

  relation prescribed_temperature on surface {
    trace(temperature) = reference_temperature;
  }

  law heat_balance on body {
    storage capacity * temperature;
    flux -conductivity * grad(temperature);
    source heating;
  }

  observable excess_energy: J =
    integral(capacity * (temperature - reference_temperature), measure(body));

  form weak_heat for heat_balance {
    test w: 1 for temperature zero_on surface;
    relation balance {
      integral(w * capacity * derivative(temperature), measure(body))
        + integral(conductivity * inner(grad(w), grad(temperature)), measure(body))
        = integral(w * heating, measure(body));
    }
  }
}

model HeatedCube(
  support body: volume(ambient_dimension = 3),
  support surface: complete_exterior(parent = body)
) {
  instance thermal: HeatedBody(
    body = body,
    surface = surface,
    capacity = 1e6 [J / (m^3 * K)],
    conductivity = 10 [W / (m * K)],
    heating = 1000 [W / m^3],
    reference_temperature = 300 [K]
  );
}
```

Bind `body` to the exact fixed cube `[0, 0.1 m]^3` and `surface` to its complete six-face
exterior using caller-owned Geometry. The exterior is a boundary set, not a volume or one
arbitrary face. Source contains no mesh or inferred coordinate-based boundary selection.
The boundary relation applies to each exact exterior member, with its parent-outward orientation.

All coefficients are spatially and temporally constant, capacity and conductivity are positive,
and the material is isotropic. Temperature is scalar. A scalar constant in an equation on a
support is the constant scalar function on that support; this does not broadcast a scalar into
a vector or change any support identity. The body is fixed: no advective transport, deformation,
or moving-boundary term is implicit in this Law.

The initial temperature and the prescribed surface temperature agree. The model supplies no
second initial derivative condition. Initial derivative or algebraic consistency, when needed,
is determined by the admitted evolution/initialization system rather than another default.

`excess_energy` is a derived private component observation, available for mathematical inspection;
it does not add an unknown or grant public import access to `temperature`. A public library
output needs an explicit interface. The form has its own mathematical identity; writing it does
not by itself certify correspondence or choose a numerical method.

## Standard package use

The proposed standard package `Eqiora.Thermal.Conduction` would export the same `HeatedBody`:

```eqiora
import Eqiora.Thermal.Conduction.conduction as thermal;

model HeatedCube(
  support body: volume(ambient_dimension = 3),
  support surface: complete_exterior(parent = body)
) {
  instance body_heat: thermal.HeatedBody(
    body = body,
    surface = surface,
    capacity = 1e6 [J / (m^3 * K)],
    conductivity = 10 [W / (m * K)],
    heating = 1000 [W / m^3],
    reference_temperature = 300 [K]
  );
}
```

The exact package must be published and locked before using this import. Its boundary behavior
remains inspectable mathematics. Alternative prescribed-flux, insulated, or convective components
must supply their actual laws; choosing a material never silently closes the exterior.

## Measures, signs, and independent checks

For a physical volume, `measure(body)` has dimension `m^3`; its exterior surface measure has
dimension `m^2`. Measures retain exact support and orientation contracts. Numerical quadrature
does not define their mathematical dimension or identity.

| Term | Dimension |
|---|---|
| `capacity * temperature` | `J/m^3` |
| `derivative(capacity * temperature)` | `W/m^3` |
| `-conductivity * grad(temperature)` | `W/m^2` |
| Divergence of physical heat flux | `W/m^3` |
| `heating` | `W/m^3` |
| Excess-energy integrand times volume measure | `J` |
| Each integrated weak-form term with dimensionless `w` | `W` |

Write `q = -conductivity * grad(T)`. The fixed-domain balance is
`capacity * dT/dt + div(q) = heating`. Integrating over the cube gives:

```text
dE/dt = heating * volume(body) - integral(q · n, surface measure)
volume(body) = (0.1 m)^3 = 0.001 m^3
heating * volume(body) = 1 W
E at initialization = 0 J
```

At steady state the total outward heat flow must be 1 W. Reversing the physical flux sign
changes this balance. A constant test integrand of `1 [J/m^3]` integrated over the same volume
must give 0.001 J, independently of the heat solution. This tests the measure without relying
on an implementation-generated temperature field.

Multiplication by a dimensionless test function and integration by parts gives the displayed
weak form. Its boundary term vanishes because `w` is zero on the entire prescribed-temperature
surface, not because heat flux is zero there. Replacing the essential boundary condition by
insulation would require changing both the boundary law and the admissible test restriction.

The initial heating with a fixed-temperature surface may have a transient corner in temporal
regularity; this specimen does not prescribe an extra pointwise boundary value for `dT/dt`.
The integral balance, initial energy, and steady outward power are the checks stated here,
not an unproved numerical convergence rate.

## Rejections

- A surface from another body, a stale handle, or missing/overlapping exterior membership fails
  exact support binding before assembly.
- Binding conductivity in `W/m^2` fails its parameter dimension check.
- A wrong integration measure changes the functional dimension or support and is rejected.
- Omitting `zero_on surface` leaves a different weak statement; correspondence must not silently
  discard its boundary term.
- Negating the source or doubling its contribution violates the independently derived 1 W input.
- A realization that cannot execute the requested form rejects it before Run, rather than
  silently replacing it with a different mathematical formulation.
