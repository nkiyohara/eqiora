# Specimen: consume a data-backed conductivity

This [target-language](core.md) specimen binds one exact property release into two components.
Constant property contracts already have a source owner; callable contracts, table-backed
releases, and the complete converged source below await their implementation slices.

## Contract and complete consumer

The following contract has one real scalar independent variable and one real scalar result.
Its identity is retained when a release is bound; a scalar with conductivity units is not a
substitute for the callable contract.

```eqiora
property contract Conductivity(input temperature: K): W / (m * K) {
  derivatives first_open_intervals;
}

component FourierFlux(
  property conductivity: Conductivity,
  input temperature: K,
  input gradient: K / m,
  output flux: W / m^2
) {
  relation constitutive {
    flux = -conductivity(temperature = temperature) * gradient;
  }
}

component SlabConductance(
  property conductivity: Conductivity,
  parameter area: m^2,
  parameter thickness: m,
  input operating_temperature: K,
  output conductance: W / K
) {
  relation constitutive {
    conductance = conductivity(temperature = operating_temperature) * area / thickness;
  }
}

model PropertyConsumers(
  property conductivity: Conductivity,
  input operating_temperature: K,
  output heat_flux: W / m^2,
  output conductance: W / K
) {
  instance local_flux: FourierFlux(conductivity = conductivity);
  instance slab: SlabConductance(
    conductivity = conductivity,
    area = 0.01 [m^2],
    thickness = 0.1 [m]
  );

  connect operating_temperature -> local_flux.temperature;
  connect operating_temperature -> slab.operating_temperature;

  relation inputs {
    local_flux.gradient = 2 [K / m];
  }
  relation outputs {
    heat_flux = local_flux.flux;
    conductance = slab.conductance;
  }
}
```

`first_open_intervals` is a closed derivative profile: value evaluation on the closed validity
interval and first derivatives on open smooth segments. Endpoint and nonsmooth-knot derivative
requests reject. This is not a promise of arbitrary higher derivatives or automatic smoothing.
The release supplies the actual segment boundaries. The contract permits no history, hidden
independent variable, uncertainty reduction, or external callback.

These are lumped constitutive evaluations, with no spatial support, boundary closure, stored
state, or initialization equation. `gradient` is a signed scalar gradient along a declared
one-dimensional local direction, not an interchangeable three-dimensional vector. The slab
conductance uses a uniform material evaluated at the supplied operating temperature; it is
not an exact nonlinear through-thickness temperature solution.

## Exact release binding

Bind `conductivity` to a synthetic instructional release with the following complete table:

| Temperature (K) | Conductivity (W/(m*K)) |
|---|---|
| 300 | 10 |
| 320 | 14 |
| 360 | 18 |

The data are exact decimal values in the specified coherent units. They describe a constructed
single-branch material, not measured values for a named substance. Scientific provenance is
this synthetic definition; redistribution follows the repository license. A published release
must retain the resolved provenance and license identities, not infer them from a display name.

Use piecewise affine interpolation of conductivity against temperature, with no logarithm,
normalization, filtering, missing-value filling, or fitted derivative table. The validity domain
is the closed interval `[300 K, 360 K]`. Value evaluation at a knot returns the tabulated value.
Outside-domain evaluation rejects; there is no extrapolation, clamping, or positivity floor.

The derivative is the segment slope strictly inside each segment. At 320 K the two slopes
differ, so a derivative request rejects. At both outer endpoints the selected profile also
rejects derivatives rather than choosing an undocumented one-sided convention. Value evaluation
there remains valid. The release exposes its derivative availability before a numerical method
that requires it is selected.

The accepted data artifact has exactly two named real-scalar columns, three rows, a strictly
increasing temperature axis, finite values, no missing entries, and the declared units. Decoder
bounds check those counts and shapes before allocation. Missing content, a mismatched digest,
duplicate or unordered abscissae, nonfinite values, and extra/missing columns reject rather than
being repaired. These are limits for this specimen's release, not universal table-size limits.

The property release binds the artifact's actual content identity through the existing artifact
owner, along with interpolation, validity, branch, derivative, and preprocessing meaning.
Large tables remain outside `.eqi`. Source refers to the release by its exact package declaration;
a mutable filename, network location, or provider default is not the accepted binding.
The [release declaration](properties.md) spells these choices with closed typed children and
an exact package asset reference, without inventing another table file format.

## Independent values and slopes

The two affine segments are:

```text
k(T) = 10 W/(m*K) + (0.2 W/(m*K^2)) * (T - 300 K),  300 K <= T <= 320 K
k(T) = 14 W/(m*K) + (0.1 W/(m*K^2)) * (T - 320 K),  320 K <= T <= 360 K
```

Both give 14 W/(m*K) at the shared knot. At 310 K, conductivity is 12 W/(m*K), the flux is
-24 W/m^2, and conductance is 1.2 W/K. At 340 K they are 16 W/(m*K), -32 W/m^2, and
1.6 W/K. The flux sign follows the stated gradient and the explicit minus sign in Fourier's
law, independently of table implementation.

Inside the first segment, `dk/dT = 0.2 W/(m*K^2)`; inside the second,
`dk/dT = 0.1 W/(m*K^2)`. Multiplication by `area/thickness = 0.1 m` gives conductance slopes
0.02 W/K^2 and 0.01 W/K^2. The endpoint conductivities are exactly 10 and 18 W/(m*K).
Tests should derive these values from the two line segments, not copy evaluator output.

## Ordinary package use and substitutions

Once the contract, release, and consumer are exported by an exact package, the short composition
uses the same property binding as any other named requirement:

```eqiora
import org.example.thermal_properties.properties as properties;

model Example() {
  instance pair: properties.PropertyConsumers(conductivity = properties.SyntheticConductivity);
  relation operating_point {
    pair.operating_temperature = 310 [K];
  }
}
```

This names the intended specimen package, not an existing published release. Its exported
contract must be the exact one used by its consumer; copying the contract under a different
package identity does not make it the same requirement.

| Substitution or request | Required outcome |
|---|---|
| A scalar `12 [W/(m*K)]` instead of the release | Wrong binding kind; no callable input contract |
| A release requiring pressure as an additional input | Incompatible independent-variable signature |
| A complex-valued or spatial-tensor conductivity | Incompatible result type |
| A foreign nominal contract with matching units | Contract identity mismatch |
| A history-dependent implementation | Incompatible purity/state contract |
| Temperature 299 K or 361 K | Validity rejection, not extrapolation |
| Derivative at 320 K | Nonsmooth-knot rejection |
| Replacement table with a changed middle value | New release meaning and content identity |
| A different interpolation or preprocessing policy | New release meaning even with identical table bytes |

Reopening the accepted dependency closure offline must reproduce the same binding. Replacing
only a conforming numerical provider preserves the declared property mathematics while changing
execution provenance; replacing data or policy is not merely provider substitution.
