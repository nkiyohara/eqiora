# Specimen: an explicit stochastic process

This target-language example uses one real Wiener channel and an Ornstein–Uhlenbeck process.
It fixes stochastic meaning without selecting a sampler or claiming current execution.
The first numerical profile is fixed-step Euler–Maruyama for admitted Ito equations.

```eqiora
model Relaxation(
  parameter rate: 1 / s = 2 [1 / s],
  parameter mean: V = 1 [V],
  parameter sigma: V / s^(1/2) = 0.5 [V / s^(1/2)],
  parameter initial_value: V = 2 [V]
) {
  noise driving: wiener(covariance = 1);
  state value: V;

  initial {
    value = initial_value;
  }
  law relaxation for value {
    calculus ito;
    drift rate * (mean - value);
    diffusion driving = sigma;
  }
}
```

The mathematical equation is `dX = rate*(mean-X) dt + sigma dW`. Rate is positive and sigma
nonnegative. Fresh time is zero and the initial value is deterministic. The process is lumped,
with no spatial support, boundary, periodic clock, or hidden reflecting/absorbing condition.
Its value ranges over the real line; “bounded profile” means finite admitted model structure,
not a claim that every sample path stays within a fixed voltage interval.

## Closed source rules

`noise name: wiener(covariance = 1);` declares one nominal real noise channel with standard
Wiener covariance: increments on disjoint time intervals are independent, mean zero, and
have variance equal to interval duration. The covariance coefficient is dimensionless.
A one-channel covariance must be nonnegative; the written unit value selects standard scaling.
This is process identity in the Model, not a mutable random-number stream.

`law name for state { ... }` selects the stochastic evolution child set: exactly one
`calculus ito;` or `calculus stratonovich;`, one drift expression, and explicitly named
diffusion-channel bindings. The target must be an owned real continuous state. A channel
appears at most once. Drift has state units divided by time, and diffusion has state units
divided by square root of time. These typed children are distinct from a conservation Law's
storage/flux/source terms; mixing the two child sets rejects.

The initial scalar profile has one channel. Multi-channel laws must bind a finite exact
channel set and explicit positive-semidefinite covariance through their admitted extension.
Two differently named channels are not automatically declared independent by their spelling.
Reusing the same exact channel in two laws deliberately shares its increments. Correlation
must not be inferred from seeds, matching units, or equal diffusion coefficients.

`noise` permits notation after its name but no `on` or `at` in this lumped profile. It cannot
appear as a pure expression yielding a fresh random draw. A stochastic Law retains ordinary
state ownership and initialization rather than creating a second execution lifecycle.

## Independent moments and dimension checks

Let `a=rate`, `mu=mean`, `s=sigma`, and `x0=initial_value`. Multiplying the equation for
`X-mu` by `exp(a*t)` gives:

```text
X(t) = mu + (x0-mu)*exp(-a*t) + s*integral_0^t exp(-a*(t-u)) dW(u)
E[X(t)] = mu + (x0-mu)*exp(-a*t)
Var[X(t)] = s^2*(1-exp(-2*a*t))/(2*a)
```

The stochastic integral has zero mean. Its variance follows by integrating
`s^2*exp(-2*a*(t-u))` over u. With the written parameters, the mean is
`(1 + exp(-(2/s)*t)) V` and the variance is
`0.0625*(1-exp(-(4/s)*t)) V^2`. The initial variance is zero; the stationary variance is
0.0625 V^2. Squaring diffusion gives V^2/s, and integrating over time gives V^2.

These are distribution statements, not predictions for one realized trajectory. A later
ensemble check must fix sample membership, uncertainty criteria, and time-discretization bias
before evaluating results. A pleasing plot or one path near the mean proves neither moment.

## Multiplicative noise and calculus

The same source owner can specify geometric Brownian motion by changing only the Law and
parameter dimensions:

```eqiora
model GeometricGrowth(
  parameter growth: 1 / s,
  parameter volatility: 1 / s^(1/2),
  parameter initial_value: V
) {
  noise driving: wiener(covariance = 1);
  state value: V;
  initial { value = initial_value; }
  law growth_law for value {
    calculus ito;
    drift growth * value;
    diffusion driving = volatility * value;
  }
}
```

For positive initial value its exact Ito path is
`X(t)=x0*exp((growth-volatility^2/2)*t + volatility*W(t))`. Mean and variance are
`x0*exp(growth*t)` and `x0^2*exp(2*growth*t)*(exp(volatility^2*t)-1)`.
Changing the source to Stratonovich with the same written drift changes the process:
the Ito conversion adds `volatility^2*X/2` to the drift. That term has units V/s.
The additive-noise relaxation example has zero correction, which alone cannot test whether
an implementation handles this distinction correctly.

Conversion must be explicit and checked through the common scalar derivative owner, or the
execution request rejects. The first fixed-step Euler–Maruyama profile does not establish
positivity for the geometric process, adaptive stepping, or arbitrary stochastic derivatives.

## Run, evaluation, and restart

Plan selects the admitted solver and sampler. Run records the realized path identity, generator
identity, and continuation state. A seed by itself does not define the same path across
different generators, channel assignments, or interval refinements. The source includes none
of these numerical choices.

Residual evaluation, differentiation of drift/diffusion, diagnostics, and observations must
not advance a random stream. Rejected numerical trials commit no state or path progress.
Refining an interval consumes the accepted path refinement contract, not unrelated replacement
increments. Exact restart retains the realized state and required path/generator continuation;
it neither resamples initialization nor replays an accepted increment.

Reject negative/non-PSD covariance, a foreign noise binding, diffusion with V/s rather than
V/s^(1/2), conflicting state owners, and an implicit calculus change. A Wiener path is not
classically time-differentiable: `derivative(value)` cannot silently replace its stochastic
Law. Differentiating smooth coefficients does not establish pathwise, weak, or distributional
sensitivities of the complete process.
