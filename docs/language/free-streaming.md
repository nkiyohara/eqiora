# Specimen: bounded 1x1v free streaming

This target-language specimen transports a scalar distribution on a product of one physical
position and one velocity coordinate. It uses general Relations and exact periodic connector
pairing; there is no special Vlasov keyword or implicit collision model. The complete product
execution path is specified, not currently established by these source examples.

```eqiora
connector StreamingBoundary {
  trace value: s / m^2;
  flux transport: 1 / m;
}

model FreeStreaming(
  support position: interval(m),
  support velocity: interval(m / s),
  parameter length: m,
  parameter speed_limit: m / s,
  parameter density: 1 / m
) {
  support phase: product(position, velocity);
  support left: boundary(parent = phase, factor = position, side = lower);
  support right: boundary(parent = phase, factor = position, side = upper);
  coordinate x: m on phase from position;
  coordinate v: m / s on phase from velocity;
  state distribution: s / m^2 on phase;
  port left_port: StreamingBoundary on left;
  port right_port: StreamingBoundary on right;

  initial {
    distribution = density / (2 * speed_limit)
      * (1 + 0.2 * math.cos(2 * math.pi * x / length));
  }
  relation transport on phase {
    derivative(distribution) + partial(v * distribution, wrt = x) = 0;
  }
  relation left_interface on left {
    left_port.value = trace(distribution);
    left_port.transport = -trace(v * distribution);
  }
  relation right_interface on right {
    right_port.value = trace(distribution);
    right_port.transport = trace(v * distribution);
  }
  connect periodic left_port, right_port;

  observable position_density: 1 / m on position = integral(distribution, measure(velocity));
  observable inventory: 1 = integral(distribution, measure(phase));
}
```

Bind position to `[0,L]`, velocity to `[-V,V]`, `length=L>0`, `speed_limit=V>0`, and a
positive line density n0. Fresh time is zero. Exact topology pairs the position endpoints
at the same velocity, preserving that factor and reversing the parent-outward position
orientation. The connection consumes this exact pairing; equal shapes or coordinate proximity
cannot establish it. The derived product boundaries retain both the selected physical endpoint
and the untouched velocity factor.

The field connector's `trace` and `flux` roles are scalar boundary quantities on that product
boundary. Their declared dimensions differ because transport is position velocity times
distribution. Both sides use the same increasing-x coordinate; the explicit minus sign on
the left supplies its outward flux. Periodic connection equates corresponding values and
balances the opposite signed fluxes, rather than enforcing zero flux separately at each end.

Acceleration and velocity-space transport are identically absent. Velocity is an exact label
along each characteristic, so neither velocity endpoint has an incoming characteristic or
needs an invented distribution boundary value. This is a finite velocity model: it claims
no Gaussian tail or infinite-domain approximation. Adding acceleration requires its velocity
flux and endpoint accounting in the same change.

## Independent solution and moments

Let `k=2*pi/L`. The exact transported distribution is:

```text
f(x,v,t) = n0/(2*V) * (1 + 0.2*cos(k*(x-v*t)))
```

It is periodic in x and reproduces the initial data. Direct differentiation gives
`partial_t f = (n0/(2*V))*0.2*k*v*sin(k*(x-v*t))` and
`v*partial_x f = -(n0/(2*V))*0.2*k*v*sin(k*(x-v*t))`, so the two terms cancel.
Both terms have units 1/m^2. A velocity derivative would instead divide f by m/s and
cannot substitute for this position derivative.

Integrating the cosine exactly over `[-V,V]` gives:

```text
n(x,t) = n0 * (1 + 0.2*cos(k*x)*sin(k*V*t)/(k*V*t)),  t != 0
n(x,0) = n0 * (1 + 0.2*cos(k*x))
integral_0^L n(x,t) dx = n0*L
```

The expression at zero uses its analytic limit, not an evaluation of zero divided by zero.
At `t=L/(2*V)`, the density is exactly n0 everywhere, although the full phase-space field
still has its transported structure. That does not imply relaxation or collisions. Total
inventory stays n0*L, and the minimum distribution is `0.8*n0/(2*V)>0`.
Continuum positivity does not establish positivity for every numerical discretization.

The spatially integrated first velocity moment is zero by symmetry. The spatially integrated
second velocity moment is `n0*L*V^2/3`, with units m^2/s^2; an average would divide by the
inventory explicitly. Neither moment is a sampled sum without a velocity measure.

## Admission and failure checks

The analytic derivative above tests coordinate units and signs. An unknown distribution keeps
its requested derivative node until an admitted product representation realizes it. A chosen
FVM reconstruction, finite-element basis, or later smooth representation has its own accuracy
and derivative availability; none changes the source into a physical 2D plate.

Reject a velocity factor substituted for position, a foreign endpoint pairing, wrong trace
units, a reversed flux without the corresponding orientation change, or a mismatched L/V
binding. Missing periodic closure must not become an absorbing or reflecting boundary.
Compare the full transported field as well as density and inventory: a solver that simply
averages in velocity can reproduce one late density snapshot while destroying the solution.

Restart retains the exact factor identities/order, periodic topology, accepted field, and time.
Refinement and numerical step choices remain Plan work. Output cadence cannot advance transport
or alter the distribution's normalization.
