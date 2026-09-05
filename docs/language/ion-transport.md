# Specimen: two-ion transport on a fixed interval

This target-language specimen is an isothermal, ideal-dilute Poisson–Nernst–Planck model with
two explicit species. It uses finite component spaces and ordinary Relations, not a chemical
parser or species-name dispatch. Complete source and numerical admission remain future slices.

```eqiora
space Ions = orthonormal(positive, negative);

model IonTransport(
  support line: interval(m),
  support ends: complete_exterior(parent = line),
  parameter length: m,
  parameter area: m^2,
  parameter diffusivity: m^2 / s,
  parameter permittivity: F / m,
  parameter faraday: C / mol,
  parameter gas_constant: J / (mol * K),
  parameter temperature: K,
  parameter reference_concentration: mol / m^3,
  parameter fixed_charge: C / m^3 = 0 [C / m^3]
) {
  coordinate x: m on line from line;
  let positive_charge: integer = 1;
  let negative_charge: integer = -1;
  state concentration: coordinates<mol / m^3, Ions> on line;
  variable flux: coordinates<mol / (m^2 * s), Ions> on line;
  variable potential: V on line;

  initial {
    concentration = coordinates(Ions, [
      reference_concentration * (1 + 0.1 * math.cos(math.pi * x / length)),
      reference_concentration * (1 + 0.1 * math.cos(math.pi * x / length))
    ]);
  }
  relation constitutive on line {
    flux.positive = -diffusivity * (
      partial(concentration.positive, wrt = x)
      + to_real(positive_charge) * faraday * concentration.positive
        * partial(potential, wrt = x) / (gas_constant * temperature)
    );
    flux.negative = -diffusivity * (
      partial(concentration.negative, wrt = x)
      + to_real(negative_charge) * faraday * concentration.negative
        * partial(potential, wrt = x) / (gas_constant * temperature)
    );
  }
  relation species_balance on line {
    derivative(concentration) + partial(flux, wrt = x) = 0;
  }
  relation poisson on line {
    -permittivity * partial(partial(potential, wrt = x), wrt = x) = faraday * (
      to_real(positive_charge) * concentration.positive
      + to_real(negative_charge) * concentration.negative
    ) + fixed_charge;
  }
  relation blocking_ends on ends {
    trace(flux) = 0;
    trace(potential) = 0 [V];
  }
  observable amounts: coordinates<mol, Ions> = area * integral(concentration, measure(line));
  observable ionic_current_density: A / m^2 on line = faraday * (
    to_real(positive_charge) * flux.positive + to_real(negative_charge) * flux.negative
  );
}
```

Bind `[0,L]` with `length=L>0`, both exact endpoints, constant positive cross-sectional area,
diffusivity, permittivity, temperature, gas constant, Faraday constant, and reference
concentration c0. Use zero fixed charge for the complete analytic binding below and fresh
time zero. Both species have the same diffusivity in this specimen. There is no advection,
reaction, reservoir, or transfer of ions across the blocking ends.

The two zero-potential endpoints are prescribed electrical boundary conditions, not a hidden
gauge repair. They can exchange displacement current with the external voltage constraint;
blocking ionic flux does not mean every electrical current is zero in a general transient.
Potential is algebraic and is initialized by Poisson plus its boundary conditions, not by an
independent guessed field. For the chosen neutral initial state that solution is zero.

## Species identity and units

The space declaration fixes species ordering; the two exact integers give its component charge
numbers through ordinary source data. Label text alone does not imply the signs. Selecting a
foreign same-sized component space rejects; reordering requires an explicit map and must
transform concentrations, fluxes, and charge metadata together.

Molar concentrations have units mol/m^3 even though the physical model varies in only one
coordinate. Area supplies the missing cross-sectional measure in each amount observation.
Without area the line integral has units mol/m^2 and is not an amount. Particle counts instead
require exact count semantics and an explicit Avogadro/volume conversion.

The migration term inside each flux has units mol/m^4, as does the concentration gradient.
Multiplying by diffusivity gives mol/(m^2*s). The Poisson left side has units C/m^3, matching
the charge density including fixed charge. Ionic current density is Faraday times signed
molar flux and has units A/m^2. Neither multiplication may silently omit Faraday's factor.

## Independent transient and equilibrium

For the complete neutral binding, write `k=pi/L`. An exact solution is:

```text
c_positive(x,t) = c_negative(x,t) = c0*(1 + 0.1*cos(k*x)*exp(-D*k^2*t))
potential(x,t) = 0
J_positive(x,t) = J_negative(x,t) = 0.1*D*k*c0*sin(k*x)*exp(-D*k^2*t)
```

Direct differentiation gives each conservation equation. Equal concentrations cancel charge
pointwise and both endpoint potentials fix the Poisson solution. Fluxes vanish at the ends.
Each species amount is exactly `area*c0*L`, ionic current density is zero, and concentrations
remain positive. The uniform long-time limit is an equilibrium. This verifies a coupled
neutral path but cannot by itself expose every migration-sign error, since its electric field
vanishes.

For a separate local equilibrium check with nonzero potential, zero species flux gives
`partial_x(log(c_i)) = -z_i*faraday*partial_x(potential)/(gas_constant*temperature)`.
Thus `c_i = C_i*exp(-z_i*faraday*potential/(gas_constant*temperature))` for positive constants
C_i. Substitution independently checks the opposite migration signs. A full nonuniform
equilibrium must also solve Poisson with its actual electrical boundary/fixed-charge data;
an arbitrary prescribed potential is not automatically such a solution.

General charge continuity follows by multiplying each species equation by its charge number
and Faraday constant. With static fixed charge,
`derivative(charge_density) + partial(ionic_current_density, wrt = x) = 0`.
Differentiating Poisson shows that total current also includes
`-permittivity*partial(derivative(potential), wrt = x)` as displacement current. Reporting
ionic current alone as total transient current would change the claim.

## Rejections and numerical boundary

Reject incompatible species/charge ordering, wrong concentration basis, foreign endpoints,
missing electrical closure, negative transport coefficients, and wrong flux dimensions.
Changing fixed charge must change the Poisson source; it cannot be dropped because a previous
neutral example passed. A new binding must recheck initial/boundary compatibility.

No positivity clipping, logarithm regularization, electroneutral substitution, or hidden
electrode reaction is admitted. Poisson-resolved transport remains distinct from an
electroneutral reduction. Mesh/time refinement and any positivity-preserving numerical method
need their own execution checks; positive analytic data alone establish no discrete guarantee.
