# Finite component spaces and maps

This section of the [target specification](core.md) chooses one source spelling for the
finite-space owner. The [two-state specimen](finite-state.md) exercises it; these constructs
are not yet compiler admission claims.

## Declarations and types

```eqiora
space Levels = orthonormal(ground, excited);
space Channels = orthonormal(command, feedback);
space Joint = product(Levels, Channels);

variable amplitude: coordinates<complex<1>, Levels>;
variable channels: coordinates<V, Channels>;
parameter coupling: map<complex<J>, Levels, Levels> =
  linear_map(Levels, Levels, [[0 [J], 2 [J]], [2 [J], 0 [J]]]);
```

`space name = orthonormal(label, ...);` declares a nonempty, finite, ordered orthonormal basis
with nominal declaration identity. Labels are unique identifiers. The written label order is
the coordinate order; file traversal and symbol-resolution order cannot alter it. Equal labels
and extent in another declaration do not establish the same space.

`space name = product(space, space, ...);` names an ordered tensor product of at least two
spaces. The name is an alias for the factor expression, not a new independent basis. Factor
order and nesting are retained; changing either requires an explicit admitted transformation.
For two factors the right factor varies fastest. `Joint` therefore orders pairs as
`(ground, command)`, `(ground, feedback)`, `(excited, command)`, `(excited, feedback)`.
A repeated factor retains distinct positions. Product extent is checked before expansion.

| Type | Meaning |
|---|---|
| `coordinates<S, B>` | Coordinates with scalar type `S` in space `B` |
| `coordinates<S, dual<B>>` | Linear dual coordinates, not implicit conjugation |
| `map<S, A, B>` | Linear map from `A` to `B`, whose matrix coefficients have scalar type `S` |

Here `S` is a real dimension or `complex<dimension>`. A map's coefficient dimension multiplies
the input coordinate dimension. For example, applying `map<J, Levels, Levels>` to dimensionless
coordinates returns energy-valued coordinates. The scalar domain embeds real into complex
when needed; it never discards an imaginary component.

An orthonormal declaration fixes its mathematical inner product, not a physical metric inferred
from an arbitrary array. Nonorthogonal metrics require their own admitted contract and reject
until supported. Numerical finite-element bases remain Realization data and are not declared
as these physical component spaces.

## Values and operations

`coordinates(B, [values])` explicitly constructs coordinates in `B`; `linear_map(A, B, rows)`
constructs a map from `A` to `B`. Map rows follow the target basis and columns the source basis.
These constructors consume exactly the declared extents. Nested array literals are input
syntax, not implicit array-to-space coercions. Their elements obey the common type rules.

For an atomic labeled space, `value.label` selects its coordinate. Product coordinates use
`coordinate(value, [factor indexes])`, with one zero-based index per atomic factor in the
declared product structure. Both access forms preserve the scalar dimension and domain.
Out-of-range or structurally mismatched selectors reject.

| Operation | Type and convention |
|---|---|
| `apply(M, x)` | Requires `x` in the exact source space of `M`; result is in its target |
| `compose(M, N)` | Apply `N` first, then `M`; the middle spaces must match exactly |
| `identity(B)` | Dimensionless real identity map on `B` |
| `inner(x, y)` | Same space; sum of `conj(x_j) * y_j` in the declared orthonormal basis |
| `pair(d, x)` | Linear evaluation of a dual coordinate on its exact primal space, without conjugation |
| `transpose(M)` | Map from the target dual to the source dual, without conjugation |
| `adjoint(M)` | Map from target to source using their admitted inner products; conjugate transpose here |
| `tensor_product(M, N)` | Map between the ordered products of the source and target factors |
| `permute_factors(x, permutation)` | Explicit coordinate permutation, retaining the reordered product type |

For vectors, `tensor_product(x, y)` produces coordinates in the ordered product space with
component `x_i * y_j`, using the same right-factor-fastest convention. It is not an outer map
with an implicitly conjugated second operand. A map transpose changes dual roles; an adjoint
does not arise just because a renderer swaps row and column labels.

Products multiply dimensions and addition requires equal dimensions and exact spaces.
No operation uses a same-sized spatial vector or raw array as a substitute. Finite products,
selectors, and permutations have static bounded extents; runtime state cannot grow topology.

## Compact grammar

```text
space-decl = "space" name [notation] "=" space-definition ";"
space-definition = "orthonormal" "(" name {"," name} ")"
                 | "product" "(" space-ref "," space-ref {"," space-ref} ")"
space-ref = qualified-name | "dual" "<" space-ref ">"
coordinate-type = "coordinates" "<" scalar-type "," space-ref ">"
map-type = "map" "<" scalar-type "," space-ref "," space-ref ">"
```

There is no `on` or `at` on a space declaration: support and activation qualify values using
the space, not the basis identity. Space declarations participate in ordinary module visibility,
forward resolution, duplicate-name checks, and cycle rejection. Product aliases do not bypass
the same finite expansion limits used for arrays and families.
