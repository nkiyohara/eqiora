# Specimen: a normalized 1D wavefunction

These complete target-language models use one fixed physical interval, homogeneous Dirichlet
endpoints, real zero potential, positive mass, and positive `hbar`. The stationary and time
paths share the same kinetic operator. This specifies source and mathematical requirements;
it does not establish current complex field or eigenproblem execution.

```eqiora
operator kinetic(
  input second_partial: complex<m^(-5/2)>,
  input mass: kg,
  input hbar: J * s
): complex<J * m^(-1/2)> = -(hbar * hbar / (2 * mass)) * second_partial;

model StationaryBox(
  support line: interval(m),
  support ends: complete_exterior(parent = line),
  parameter mass: kg,
  parameter hbar: J * s
) {
  coordinate x: m on line from line;
  variable psi: complex<m^(-1/2)> on line;
  variable energy: J;

  relation endpoint_values on ends {
    trace(psi) = 0;
  }
  relation eigen_equation on line {
    kinetic(second_partial = partial(partial(psi, wrt = x), wrt = x), mass = mass, hbar = hbar)
      = energy * psi;
  }
  relation unit_norm {
    integral(math.abs2(psi), measure(line)) = 1;
  }
  form spectrum for eigen_equation {
    eigenpair(mode = psi, value = energy, metric = l2(measure(line)), normalization = unit_norm);
  }
}

model EvolvingBox(
  support line: interval(m),
  support ends: complete_exterior(parent = line),
  parameter length: m,
  parameter mass: kg,
  parameter hbar: J * s
) {
  coordinate x: m on line from line;
  state psi: complex<m^(-1/2)> on line;

  initial {
    psi = math.sqrt(1 / length)
      * (math.sin(math.pi * x / length) + math.sin(2 * math.pi * x / length));
  }
  relation endpoint_values on ends {
    trace(psi) = 0;
  }
  relation evolution on line {
    math.i * hbar * derivative(psi) =
      kinetic(second_partial = partial(partial(psi, wrt = x), wrt = x), mass = mass, hbar = hbar);
  }
  observable density: 1 / m on line = math.abs2(psi);
  observable probability: 1 = integral(math.abs2(psi), measure(line));
}
```

Bind `line` to the fixed physical interval `[0, L]`, with `L > 0`, and `ends` to both exact
endpoints. In the time model bind `length` to that same L; mismatched length and Geometry
do not satisfy this specimen's initial/boundary contract. Set fresh time to zero. No spatial
periodicity, absorbing endpoint, stochastic path, or numerical grid is implicit. `kinetic`
is a pure scalar operation evaluated pointwise on the argument's support; it neither reads
an ambient coordinate nor differentiates an argument implicitly.

Each spatial derivative divides wave amplitude dimensions by length. Thus the second partial
has dimension `m^(-5/2)`, the kinetic term has `J*m^(-1/2)`, and `i*hbar*derivative(psi)`
has that same dimension. Real initial data embeds into complex values without losing units.
The density is `1/m`, so its line integral is dimensionless.

## Eigenpair and normalization roles

`eigenpair(...)` is a closed form child. Its `mode` names the exact unknown, `value` names the
real scalar eigenvalue, `metric` names the mathematical inner product, and `normalization`
references an explicit normalization relation. All four bindings are required and unique.
They do not create another mode, eigenvalue, or copy of the normalization equation.

`l2(measure(line))` denotes the inner product integrating `conj(a)*b` with that exact measure;
`euclidean(space)` denotes the declared orthonormal finite-space metric. These are typed
metric constructors for this child, not user-defined strings or numerical mass-matrix guesses.
The referenced relation supplies the norm target. Zero vectors are excluded by the written
unit norm, not by a solver starting vector.

The form includes the mode's original endpoint restrictions and all dependencies needed by
the selected eigen-equation. Missing boundary closure, an indefinite/singular unhandled metric,
an incompatible eigenvalue dimension, or an unsupported Hermitian profile rejects. Numerical
mass matrices represent the metric; normalizing raw nodal coefficients in Euclidean norm does
not replace it. Mode count, shifts, search ranges, and algorithms stay in the study/Plan request.

For this real zero-potential Dirichlet operator, integration by parts on its admitted domain
establishes self-adjointness. The source form requests that profile; a later potential or
boundary replacement must satisfy the same actual admission checks.

## Independent stationary and time solutions

For positive integer n, differentiation of `sin(n*pi*x/L)` gives:

```text
phi_n(x) = sqrt(2/L) * sin(n*pi*x/L)
E_n = hbar^2*pi^2*n^2/(2*mass*L^2)
integral_0^L |phi_n|^2 dx = 1
```

The endpoint values are zero and distinct modes are orthogonal. The n=0 function is zero
and fails normalization; it is not a ground state. A constant potential shift V0 would add
V0 to every energy while preserving these modes, but must be explicitly present in the model.

The time model starts from `(phi_1 + phi_2)/sqrt(2)`. Its exact solution is
`(phi_1*exp(-i*E_1*t/hbar) + phi_2*exp(-i*E_2*t/hbar))/sqrt(2)`.
Substituting this expression into the evolution equation fixes the sign and the hbar factor.
Its probability is one and mean energy is `(E_1 + E_2)/2 = 5*E_1/2`.

The density is
`(phi_1^2 + phi_2^2)/2 + phi_1*phi_2*cos((E_2-E_1)*t/hbar)`.
At `x = L/4`, it starts at `(3/2 + sqrt(2))/L`. At
`t = pi*hbar/(E_2-E_1)` it is `(3/2 - sqrt(2))/L`. This interference change is an independent
relative-phase check. Probability alone would not detect all phase errors. Reversing time
phase also leaves this real-initial-data density unchanged, so signed complex amplitude or
probability-current checks are additionally necessary to expose a flipped evolution sign.

An accepted restart retains the complex field, exact interval/boundary identities, and timeline
coordinate. It must not reapply the initial superposition or renormalize a numerically drifted
state. Output cadence does not set the integrator's physical steps. Spatial and temporal
accuracy require the selected method's own checks, independently of source normalization.

Wrong wave-amplitude dimensions, missing endpoints, a kinetic sign flip, absent hbar, and
Euclidean nodal normalization each violate a different stated requirement. Tests should reach
those requirements directly rather than compare arbitrary eigenvector phases or full files.
