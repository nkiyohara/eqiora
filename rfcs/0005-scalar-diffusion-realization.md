# RFC 0005: Scalar diffusion numerical realization

- Status: Draft implementation
- Authors: Eqiora contributors
- Created: 2026-07-17

## Summary

Eqiora's first spatial numerical kernel is a generic constant-coefficient 1D
diffusion operator on a uniform line, advanced by centered second differences,
Crank–Nicolson integration, and a tridiagonal solve.

## Motivation

The architecture needs early, falsifiable evidence that numerical kernels can
have explicit approximation contracts without leaking those choices into the
Semantic Kernel. Adding a heat-specific semantic node would produce a quick
demo but would make the canonical model depend on one physics label and one
discretization. Claiming canonical PDE support before shape, representation,
and spatial lowering are defined would be equally misleading.

The scalar diffusion equation supplies a small test with a known solution and
a measurable convergence order:

```text
u_t = alpha u_xx + s(x, t),  alpha > 0
```

## Proposed design

`eqiora-numerics` is an L3 crate. It receives resolved scalar values and owns
approximation choices; it cannot change graph meaning.

`UniformLine` defines finite increasing endpoints and at least two equal
intervals. `Diffusion1d` holds that grid and a finite positive diffusion
coefficient. One stateless step accepts:

- the full nodal state at time `t`;
- a finite positive `dt`;
- a source function `s(x, t)`;
- time-dependent left and right Dirichlet values.

For `r = alpha dt / h^2`, each interior row uses

```text
-r/2 u^(n+1)_(i-1) + (1+r) u^(n+1)_i - r/2 u^(n+1)_(i+1)
  = r/2 u^n_(i-1) + (1-r) u^n_i + r/2 u^n_(i+1)
    + dt/2 (s_i^n + s_i^(n+1)).
```

New-time boundary values move to the right-hand side. The resulting
tridiagonal system is solved by checked Thomas elimination. Invalid grids,
coefficients, times, states, sources, and boundary data produce `EQ0801`;
singular or non-finite solves produce `EQ0802`.

This is a numerical API, not a canonical spatial-model API. It introduces no
kernel node and makes no claim that Eqiora Language can yet express a PDE.

## Alternatives considered

### Add a heat-equation kernel node

This would confuse a domain equation with canonical relation semantics and
would not generalize cleanly to mass, momentum, species, or arbitrary
operators. Rejected.

### Reuse scalar expression DAGs for every grid operation

Expanding stencils into scalar DAGs would permit a prototype, but it discards
shape, locality, sparsity, and boundary structure before Operator IR can use
them. Deferred until the tensor/spatial IR contract is designed.

### Explicit Euler

Simpler per step, but its parabolic stability restriction would entangle a
basic accuracy test with step rejection and default-schedule policy.
Crank–Nicolson is selected as a compact second-order temporal baseline. This
does not establish it as the future universal default.

## Compatibility and migration

The crate is new and pre-alpha. Its API may be replaced by spatial Operator IR
lowering without changing the nine canonical node kinds. Diagnostic codes are
append-only. A later default-realization policy must name its numerical
contract and artifact version explicitly.

## Verification

- Reject invalid grids, diffusion coefficients, state sizes, and step data.
- Preserve a linear steady profile to roundoff.
- For `u(x,t) = sin(pi x) exp(-alpha pi^2 t)` on `[0,1]`, refine 10, 20, and
  40 intervals while using `dt = 0.2 h^2 / alpha` so temporal error is
  higher-order than spatial error.
- Require monotonically decreasing L2 error and both observed refinement rates
  to exceed 1.9 in CI.

## Security, safety, and governance

The implementation uses safe Rust and allocates in proportion to the validated
grid size. User callbacks execute synchronously and every returned scalar is
checked for finiteness. This draft does not select a project-wide default
method; accepting such a policy still requires RFC consensus.

## Unresolved questions

- Canonical tensor shape and `Representation` contracts.
- Boundary and source semantics in Eqiora Language.
- Spatial Operator IR instructions and sparse-memory layout.
- Default-realization selection, stability policy, adaptivity, and artifacts.
- Nonuniform grids, variable/tensor coefficients, and higher dimensions.
