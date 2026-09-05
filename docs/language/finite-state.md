# Specimen: a two-state Hamiltonian

This complete target-language model uses [finite component spaces](finite-spaces.md), complex
scalars, ordinary state evolution, and derived observations. Its source and execution depend
on the corresponding language and runtime slices; no quantum-specific executor is implied.

```eqiora
space Levels = orthonormal(ground, excited);

model TwoState(
  parameter energy_scale: J,
  parameter hbar: J * s
) {
  let hamiltonian: map<complex<J>, Levels, Levels> =
    linear_map(Levels, Levels, [[0 [J], energy_scale], [energy_scale, 0 [J]]]);
  state psi: coordinates<complex<1>, Levels>;

  initial {
    psi = coordinates(Levels, [math.complex(1, 0), math.complex(0, 0)]);
  }
  relation evolution {
    math.i * hbar * derivative(psi) = apply(hamiltonian, psi);
  }

  observable ground_probability: 1 = math.abs2(psi.ground);
  observable excited_probability: 1 = math.abs2(psi.excited);
}

model StationaryTwoState(parameter energy_scale: J) {
  let hamiltonian: map<complex<J>, Levels, Levels> =
    linear_map(Levels, Levels, [[0 [J], energy_scale], [energy_scale, 0 [J]]]);
  variable mode: coordinates<complex<1>, Levels>;
  variable energy: J;
  relation stationary {
    apply(hamiltonian, mode) = energy * mode;
    inner(mode, mode) = 1;
  }
}
```

The binding requires positive real `energy_scale` and `hbar`. The basis is explicitly
orthonormal. Both models are lumped and continuous; they have no spatial support, boundary,
or clock. `psi` is dimensionless because its finite-state norm is a sum, not an integral over
length. An independently normalized 1D spatial wavefunction instead has dimension `m^(-1/2)`.

The Hamiltonian is time independent and Hermitian: its real off-diagonal entries agree and
its diagonal entries are zero. The physical equation retains `hbar` with dimension `J*s`.
Both sides have energy-valued coordinate type. No assumption that energy is an angular
frequency is hidden in a package or solver.

The stationary model defines its nonzero mode through the normalization equation, not an
initial guess. It has no evolution state or fresh-time initialization. A spectral Formulation
must identify the eigenpair and metric through its typed owner; a generic square-system solver
must not infer an eigenproblem merely from the spelling of this relation. Mode count, shifts,
tolerances, and algorithms remain requests outside these Model equations.

## Independent solution

Write `e = energy_scale`, `b = hbar`, and measure `t` from fresh initialization. The exact
solution is:

```text
psi.ground(t)  = cos(e*t/b)
psi.excited(t) = -i*sin(e*t/b)
P_ground(t)    = cos(e*t/b)^2
P_excited(t)   = sin(e*t/b)^2
```

Substitution proves both component evolution equations and the initial vector. Total
probability is one and energy expectation is zero. At `t = pi*b/(2*e)` the excited probability
is one; at `t = pi*b/e` it is zero and the state is minus the initial vector. Those two vectors
have the same component probabilities but are not the same exact State artifact.

The stationary eigenvalues are `+e` and `-e`. Corresponding normalized coordinates are
`[1, 1]/sqrt(2)` and `[1, -1]/sqrt(2)`. Their inner product is zero, and direct multiplication
by the written matrix yields the respective eigenvalue times each vector. Multiplication by
any common unit-modulus complex phase preserves each physical eigenmode. Verification should
compare residuals, normalization, and phase-invariant projectors, not require one raw vector.

Changing `energy_scale` changes the frequency and eigenvalues. Removing `hbar` is a dimension
error, not an alternative convention. Flipping the sign of `math.i` in evolution changes the
phase trajectory even though these two probabilities alone cannot detect it; the complex
component equations or phase-sensitive observations must expose that mutation.

## Independent space consumers

The same map grammar also types two unrelated voltage channels:

```eqiora
space Channels = orthonormal(command, feedback);
parameter gains: map<1, Channels, Channels> =
  linear_map(Channels, Channels, [[2, 0], [0, 3]]);
let input_channels: coordinates<V, Channels> = coordinates(Channels, [1 [V], 2 [V]]);
let output_channels: coordinates<V, Channels> = apply(gains, input_channels);
```

The result is `[2 V, 6 V]`. Substituting `Levels` coordinates fails despite equal extent.
Neither space can be replaced by `array<V, 2>` or a spatial `vector<V, 2>`.

For a two-factor example, declare two distinct two-state spaces and retain their order:

```eqiora
space Left = orthonormal(ground, excited);
space Right = orthonormal(ground, excited);
space Pair = product(Left, Right);
let left_h: map<J, Left, Left> = linear_map(Left, Left, [[0 [J], 1 [J]], [1 [J], 0 [J]]]);
let right_h: map<J, Right, Right> = linear_map(Right, Right, [[0 [J], 2 [J]], [2 [J], 0 [J]]]);
let pair_h: map<J, Pair, Pair> =
  tensor_product(left_h, identity(Right)) + tensor_product(identity(Left), right_h);
```

Its four eigenvalues are the pairwise sums `3 J, -1 J, 1 J, -3 J`; eigenvectors are products
of each factor's two normalized eigenvectors. Applying it to `[1, 0, 0, 0]` in the declared
right-factor-fastest order gives `[0, 2 J, 1 J, 0]`. Swapping factors requires an explicit
permutation, not reusing the old coordinates under another display label. This bounded
four-coordinate example makes no efficient many-body expansion assumption.
