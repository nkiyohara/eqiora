# Reference construction

The numerical reference is the Galerkin weak form of inertial Stokes flow and
first-order small-strain elastodynamics on one fixed conforming triangular
mesh. Backward Euler eliminates the next displacement:

```text
d_s^(n+1) = d_s^n + dt * v_s^(n+1).
```

The velocity block is the sum of fluid mass and viscous actions plus solid
mass and `dt`-scaled elastic actions. Fluid and solid P1 interface velocities
share the same quotient rows; the MINI cell bubble has zero trace. Pressure
couples only on fluid cells. The independently evaluated constant-pressure
action on the free interface proves that the coupled anchored solid closes the
pressure mode, so no algebraic gauge is admitted.

For zero external work, the reference backward-Euler identity is

```text
E_next - E_previous
  + 1/2 ||u_next - u_previous||^2_Mf
  + 1/2 ||v_next - v_previous||^2_Ms
  + 1/2 ||d_next - d_previous||^2_Ks
  + dt * a_fluid(u_next, u_next)
  = 0.
```

The implementation assembles the dimensionless local actions directly under
the RFC 0045 positive symmetric congruence. The captured canonical CSR is the
sole executable operator and the sole operator identity intended for later
CUDA reuse.
