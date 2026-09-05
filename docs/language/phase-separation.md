# Specimen: conserved binary phase separation

This target-language example uses a fixed physical line and a dimensionless signed order
parameter. Its energy is a one-dimensional effective energy: bulk density has units J/m,
not the J/m^3 of a three-dimensional material. The source, variational correspondence, and
numerical execution remain distinct admission steps.

```eqiora
model PhaseSeparation(
  support line: interval(m),
  support ends: complete_exterior(parent = line),
  parameter length: m,
  parameter bulk: J / m,
  parameter gradient: J * m,
  parameter mobility: m^3 / (J * s)
) {
  coordinate x: m on line from line;
  state composition: 1 on line;
  variable chemical_potential: J / m on line;

  initial {
    composition = 0.01 * math.cos(math.pi * x / length);
  }
  observable free_energy: J = integral(
    bulk * (composition^2 - 1)^2 / 4
      + gradient * inner(grad(composition), grad(composition)) / 2,
    measure(line)
  );
  relation chemical on line {
    chemical_potential = bulk * (composition^3 - composition)
      - gradient * partial(partial(composition, wrt = x), wrt = x);
  }
  law conserved_composition on line {
    storage composition;
    flux -mobility * grad(chemical_potential);
    source 0;
  }
  relation closed_ends on ends {
    normal_trace(grad(composition)) = 0;
    normal_trace(-mobility * grad(chemical_potential)) = 0;
  }
  form chemical_variation for chemical {
    test eta: 1 for composition;
    relation definition {
      integral(chemical_potential * eta, measure(line)) =
        variation(free_energy, wrt = composition, direction = eta, holding = (bulk, gradient));
    }
  }
}
```

Bind the line to `[0,L]`, `length=L>0`, both exact endpoints, and positive constant bulk,
gradient, and mobility coefficients. The initial cosine and its induced chemical potential
have zero normal derivatives at both endpoints. Chemical potential is algebraic and gets no
independent initial value. There is no imposed flux, external energy input, clock, or hidden
concentration clipping. The signed order parameter is not a species count or mass fraction.

The physical flux has units m/s: mobility times the gradient of chemical potential gives
`m^3/(J*s) * J/m^2`. Its divergence has units 1/s, agreeing with the storage derivative.
Conserved inventory is the line integral of composition, with units m, not particle number.
It is initially zero for the specified cosine.

## Variation and boundary terms

`variation(functional, wrt = field, direction = perturbation, holding = (...))` denotes a
directional first variation. Its direction has the field's dimension, support, and admissible
boundary restrictions. The result has the functional's dimension, here J. `holding` is a
compile-time set of exact independent bindings; the selected measure and domain are fixed.
A second variation nests this operation with a second independent direction and retains
their order. It does not substitute an ordinary parameter Jacobian for a field variation.

Writing c for composition and eta for its direction gives the independent variation:

```text
delta F[c;eta] = integral (bulk*(c^3-c)*eta + gradient*c_x*eta_x) dx
              = integral (bulk*(c^3-c)-gradient*c_xx)*eta dx
                + [gradient*c_x*eta] at the endpoints
```

The boundary term vanishes here because the actual boundary law sets the normal derivative
of c to zero. The test function is not silently constrained to zero. Dropping the boundary
term before checking that condition would assert a different correspondence. The functional
derivative density has units J/m, whereas its directional integral has units J. A functional
gradient would additionally need a declared pairing/Riesz map.

The form refers to the existing chemical relation and records its weak variational statement;
it does not add another physical evolution equation or minimize the observable automatically.
Mobility and conservation are separate physical choices. Nonconserved Allen–Cahn relaxation
would instead use `derivative(c) = -L_ac*chemical_potential`, with `L_ac` in m/(J*s), and
would not conserve the composition inventory.

## Independent growth and dissipation checks

For a small cosine perturbation about homogeneous c0, with wave number k=n*pi/L and n>0,
linearizing the written chemical relation gives the growth rate
`lambda = -mobility*k^2*(bulk*(3*c0^2-1) + gradient*k^2)`.
Near c0=0, sufficiently long waves grow; a wrong sign in either mixed equation changes this
rate. This linear prediction is not an exact finite-amplitude solution of the cubic model.

Multiplying the conservation equation by chemical potential and using both closed-end laws
gives `dF/dt = -integral mobility*|grad(chemical_potential)|^2 dx <= 0`.
Integrating conservation without that multiplier gives constant composition inventory.
These are separate continuum identities. A chosen numerical method must establish its own
discrete conservation/energy behavior; nonlinear solve success or an attractive pattern does
not establish either one.

As an independent quadratic-potential variation, replace the bulk density by `bulk*c^2/2`
and its chemical term by `bulk*c` together. Then the cosine amplitude decays exactly as
`exp(-mobility*k^2*(bulk+gradient*k^2)*t)` and the gradient boundary condition remains valid.
Changing only the observable or only the chemical relation fails their correspondence.

Reject foreign endpoints, mismatched line length, nonpositive coefficients for this profile,
missing boundary flux closure, a held-fixed dependent field, and incompatible perturbation
units. The mixed system uses existing conservation and form owners, not a phase-field solver
selected by model name. Mesh, time method, and block solver remain Plan choices.
