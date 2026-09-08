# Typed polynomial operators

`operator` declares a bounded, side-effect-free mathematical definition. Inputs and the
result carry their types, and calls bind arguments by formal name:

```eqiora
operator conductivity(
  input delta_temperature: K,
  input k0: W / (m * K),
  input a: 1 / K
): W / (m * K) =
  k0 * (1 + a * delta_temperature + a * a * delta_temperature^2);
```

With a temperature difference of 20 K, `k0 = 10 W/(m*K)` and `a = 0.01/K`, the
result is 12.4 W/(m*K). Both the constant and temperature-dependent terms have the
same complete dimensions. A call uses
`conductivity(delta_temperature = temperature, k0 = coefficient, a = curvature)`;
authored named-argument order does not change the formal binding.

Concrete ordinary real scalar signatures coexist with the existing operator-local generic
`scalar` and `spatial[rank]` classes. Those generic classes remain dimension-polymorphic;
`scalar` is not a synonym for the dimensionless concrete type `1`. Existing exact tensor
bodies retain `rational`, `component` and `delta`. The former `pure operator` and `->`
declaration spellings are not part of the current source surface.

Polynomial bodies use exact decimal or rational coefficients, formal references and bounded
arithmetic. Powers use positive integer literal exponents up to 255 and share the
calculus work and depth limits. Zero, negative or computed exponents are outside this
profile; write the constant `1` for a degree-zero term. This preserves the general value-power
rule that `0^0` rejects instead of erasing its domain check during polynomial expansion.
Calls between local scalar operators compose through the existing exact calculus owner. Lexical names and formal bindings are resolved before expansion; cycles, capture,
unknown or missing arguments, incompatible types and excess work reject locally. Operators
cannot read hidden evolving state or invoke host callbacks. Cross-package composition inside
operator definitions and the general analytic/complex catalog remain outside this profile;
existing qualified package calls at Model and Component use sites retain their visibility rules.

Definition identity retains exact arithmetic and input/result scalar-domain and dimension
constraints. The
current Model stores the operator definition and application identity. Execution projects an
admitted scalar application into the ordinary expression graph and uses the existing evaluator;
proof normalization does not authorize reassociating floating-point operations.

The native scalar calculus owner exposes inspectable first and second partial derivative
graphs. For the example, the temperature derivatives at 20 K are 0.14 W/(m*K^2) and
0.002 W/(m*K^3). A zero partial retains its derived dimensions and support. Unsupported
orders or profiles reject rather than substituting finite differences. General source-level
`partial`, mixed derivative products and solved sensitivities follow the separate
[calculus contract](calculus.md); this operator profile does not claim them.

Python `Source.operator` authors a local concrete scalar definition from typed formal handles
and a callback invoked once. Its returned handle accepts named arguments. Formal handles
cannot escape their scope, and a foreign Source cannot acquire a call merely because every
argument is a literal. Qualified imported operator calls remain available through source
compilation rather than a raw qualified-name handle constructor.
