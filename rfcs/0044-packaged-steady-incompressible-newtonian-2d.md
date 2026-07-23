# RFC 0044: Packaged steady incompressible Newtonian 2D law

- Status: Accepted; bounded semantic and package implementation verified
- Authors: Eqiora contributors
- Created: 2026-07-20
- Depends on: [RFC 0022](0022-exact-package-identity-and-resolution.md),
  [RFC 0034](0034-occurrence-bound-spatial-supports.md),
  [RFC 0038](0038-canonical-tensor-structure-operators.md), and
  [RFC 0040](0040-occurrence-bound-field-slots.md)
- Related numerical gate: [RFC 0043](0043-simplicial-mini-stokes-realization.md)

## Summary

Eqiora admits one exact reusable steady incompressible Newtonian fluid law as
the public
`Eqiora.Fluid.Incompressible@0.1.0::SteadyStokesWithPotential2d`
Component. An occurrence binds one two-dimensional volume, velocity, pressure,
and conservative-force-potential Field plus one constant dynamic-viscosity
Parameter. It expands into only the ordinary canonical Relations

```text
-div(2 mu symmetric_part(grad(u)) - isotropic_lift(p)) - grad(q) = 0,
 div(u) = 0.
```

The package owns continuous physics only. The enclosing root owns the concrete
Domain and Fields, the definition of `q`, and the complete zero-velocity trace.
Direct-flat and exact-package forms pass the same whole-Model,
name-independent semantic recognizer after a verification-private identity
normalization.

This RFC adds no Kernel node, expression primitive, package schema, Model wire,
mesh, mixed space, gauge, solver, or execution policy. In particular, it does
not connect this dimensional canonical law to the separate nondimensional MINI
execution slice in RFC 0043.

## Motivation

RFC 0043 proves that the numerical contracts can execute one stable mixed
velocity--pressure discretization. It intentionally begins from numerical
callables, so it cannot prove that a reusable fluid package has the same
meaning as an explicit canonical Model. Conversely, putting MINI, a pressure
gauge, or MINRES into a fluid package would make numerical method part of
physical meaning and break the Semantic Model / Realization boundary.

The smallest independent semantic test is therefore:

```text
exact package Component
        |
        | support, Field, and Parameter occurrence binding
        v
ordinary flat Domain / Field / Parameter / Relation network
        |
        | one name-independent whole-Model recognizer
        v
method-neutral steady incompressible Newtonian law
```

The conservative-force potential `q` is deliberately narrower than a general
vector body-force Field. Its gradient uses the existing typed spatial
expression vocabulary and gives a falsifiable sign convention without adding
a second vector-Field definition mechanism. General body forces can be added
later as an independent language and lowering extension.

## Exact semantic boundary

### Types and support

The admitted Model has one exact two-dimensional Cartesian volume `Omega` and
three continuum Fields on that same support:

```text
u : velocity,        m / s,              shape [2], SpatialCartesian
p : pressure,        kg / (m * s^2),      shape [],  Invariant
q : force potential, kg / (m * s^2),      shape [],  Invariant
mu: dynamic viscosity, kg / (m * s),      scalar constant Parameter
```

All Fields must use the same continuum Representation identity. `mu` must be
finite and strictly positive. The resulting momentum residual has dimension
`kg / (m^2 * s^2)` and shape `[2]`; the incompressibility residual has
dimension `1 / s` and scalar shape. Both Relations are continuously activated
on `Omega`.

The root defines `q` through one ordinary supported scalar Relation

```text
q - q_hat(coordinate, scalar_parameters) = 0,
```

where `q_hat` is accepted by the existing scalar spatial-expression contract.
The package does not own that loading function. Constant additions to `q`
have no effect on momentum, but its exact defining Relation remains Model
meaning and identity.

### Momentum and incompressibility

Define

```text
epsilon(u) = symmetric_part(grad(u)),
tau(u)     = 2 mu epsilon(u),
sigma(u,p) = tau(u) - isotropic_lift(p).
```

The Component contributes exactly

```text
-div(sigma(u,p)) - grad(q) = 0,
 div(u) = 0.
```

This fixes the factor two, symmetric rather than full velocity gradient,
pressure sign, outer divergence, force-potential sign, and separate scalar
continuity Relation. The recognizer accepts only this stated structural normal
form; it does not guess a fluid law from names or perform general symbolic
equivalence.

Pressure is determined only up to a constant by this continuous closed
problem. That nullspace is mathematical truth, not an incomplete Semantic
Model. Selecting a representative belongs to a Realization and is therefore
not expressed by a pressure pin or mean constraint here.

### Boundary closure

The root owns the four exact boundaries of its Cartesian volume and one
ordinary Relation on each:

```text
trace(u) = 0.
```

The Component has no exterior-support slot, Connector, Port, boundary family,
or terminal. Missing, duplicate, nonzero, natural, open, or unrelated boundary
closure falls outside this first whole-Model subset. This closed benchmark
proves a reusable volume law; it does not pre-empt the distinct public fluid
boundary and FSI contracts.

## Package contract

The exact package exports one Component with the conceptual source surface

```text
public component SteadyStokesWithPotential2d {
  public support body: volume(ambient_dimension = 2);
  public field slot velocity on body as continuum:
    m / s shape spatial_vector;
  public field slot pressure on body as continuum:
    kg / (m * s ^ 2);
  public field slot force_potential on body as continuum:
    kg / (m * s ^ 2);
  public parameter dynamic_viscosity: kg / (m * s);

  relation momentum continuous on body {
    -div(
      2 * dynamic_viscosity * symmetric_part(grad(velocity))
      - isotropic_lift(pressure)
    ) - grad(force_potential) = 0;
  }

  relation incompressibility continuous on body {
    div(velocity) = 0;
  }
}
```

This source sketch records the public contract, not a new wire schema. Existing
occurrence-bound support and Field slots specialize the exact outer Field
identities before expansion. The slots disappear; both Relations refer
directly to the root-owned Fields in the resulting ordinary flat graph.

The package semantic digest covers compiler-derived release meaning under the
existing exact offline contracts. Dependency alias, provider label, source
file, declaration order, Relation order, Field-binding order, and Parameter-
binding order are not semantic dispatch keys. Changing a bound Field, support,
viscosity value, or Relation changes semantic meaning rather than being
normalized away.

## Method-neutral recognition

One whole-Model recognizer admits either an explicit-flat network or the
ordinary flattened result of package elaboration. It derives identity from
the committed graph and never branches on package, Component, provider, file,
or author symbol names.

The recognized value contains only:

- exact volume, velocity, pressure, and force-potential Field identities;
- the physical Cartesian bounds and shared continuum Representation;
- the finite positive constant `mu` value;
- the immutable scalar spatial tape defining `q`;
- proofs of the exact momentum and incompressibility structures; and
- proof of the complete homogeneous velocity trace.

It contains no mesh, quadrature, basis, discrete block order, stabilization,
pressure gauge, congruence scale, sparse matrix, solver, target, schedule, or
package identity. Extra Domains, Fields, Relations, Activations, Connections,
or unrelated graph structure fail rather than being silently ignored. This
strictness makes direct/package equivalence a statement about the complete
accepted Model, not a match found inside a larger graph.

## Falsifying verification

The registered
[`fluid.packaged-steady-stokes-2d`](../verify/fluid/packaged-steady-stokes-2d/README.md)
case owns this bounded claim. It must prove:

1. one explicit-flat Model and the exact-package form produce the same
   recognized meaning after a complete, verification-private identity
   bijection;
2. package alias, provider label, source-file order, declaration order,
   Relation order, and binding order do not change normalized meaning;
3. velocity, pressure, and force-potential dimensions, shapes, frames,
   Representation, and exact support identities are checked independently;
4. `mu` has the dynamic-viscosity dimension and is finite and positive;
5. momentum retains the exact pressure sign, factor two, symmetric gradient,
   outer divergence, force-potential sign, and vector residual type;
6. incompressibility remains one separate scalar `div(u) = 0` Relation;
7. the load-potential definition and all four zero velocity traces are present;
8. an unrelated node or otherwise valid extra Relation makes whole-Model
   recognition fail closed; and
9. exact release preparation, retained installation, resolution, compilation,
   and replay preserve the existing package and compilation identities.

Negative fixtures independently perturb support dimension, Field shape,
Field frame, physical dimension, support or Representation identity, viscosity
dimension/value, each structural operator/sign/factor, continuity, load
definition, and boundary completeness. Package names are changed in positive
fixtures and therefore cannot be recognition keys.

## Alternatives considered

### Package the numerical MINI realization

This would make mesh, mixed-space, quadrature, gauge, and solver choices part
of a continuous-physics dependency. It would also falsely imply that the
dimensional canonical Fields already map to a public field-wise mixed
Realization. Rejected; RFC 0043 remains an independent numerical gate.

### Bind a general vector body-force Field

A vector force is more general, but it needs a comparably typed definition and
lowering path before it is a better public contract. The scalar potential uses
existing spatial-expression differentiation and is enough to falsify the
momentum sign. Deferred rather than rejected generally.

### Let the Component own Fields, geometry, or boundary conditions

That would turn a reusable material law into one benchmark topology and make
multiple occurrences unable to bind distinct enclosing objects cleanly.
Rejected in favor of exact occurrence obligations.

### Put a pressure gauge in the Semantic Model

A point pin or mean constraint selects a numerical representative of the same
continuous pressure equivalence class. Encoding it here would confuse meaning
with realization and make package identity depend on discretization policy.
Rejected.

### Add a fluid-specific Kernel node or recognize the package name

Both alternatives create parallel semantics and make another authoring surface
capable of changing Model meaning without changing the ordinary Relation
network. Rejected.

## Explicit nonclaims

This RFC does not claim numerical Stokes execution from this Model, a public
field-wise Realization v2, unit-consistent symmetric-indefinite congruence
scaling, a pressure-gauge Realization, solver/artifact support for the mixed
system, fluid boundary Ports, natural or open boundaries, nonconservative body
forces, transient or Navier--Stokes flow, ALE, turbulence, FSI, or a broad
fluid component library.

Those are separate evidence gates. In particular, the existence of both this
semantic/package case and RFC 0043's numerical case must not be described as
an end-to-end canonical Stokes solve until an explicit Realization contract
connects them.
