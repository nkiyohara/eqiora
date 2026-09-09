# Explicit formal partials

The case admits real scalar polynomial partials and local operator composition at named
independent bindings, without a mesh or solved-state sensitivity. Compiler tests preserve
Parameter alias dependencies and reject dependent, foreign, conflicting, and unsupported
bindings or rules. Installed Python product tests separately cover direct authoring, emitted
source, and Model artifact replay.

Independent values are derived from the product and chain rules: for `f=x*x*y`, at `x=3,y=5`,
`f_x=30`, `f_y=9`, and `(f*f)_x=2700`. A let alias `z=x*x` has `z_x=6` and `z_y=0` even
when the independently declared Parameters happen to have equal values. For
`k=k0*(1+a*(T-T0)+a*a*(T-T0)*(T-T0))`, with `k0=10`, `a=0.01`, and `T-T0=20 K`,
`k_T=0.14 W/(m*K^2)`. The test tolerance is `1e-15` in these coherent units for the
short binary64 polynomial, independent of implementation output. Temperature subtraction
uses ordinary scalar Kelvin values; affine absolute-temperature typing is not claimed.

This case does not establish complex, nonsmooth, coordinate, material, implicit-solution,
or arbitrary higher-order derivatives.

Run `cargo run -p eqiora-verify -- run --case language.formal-partials`.
