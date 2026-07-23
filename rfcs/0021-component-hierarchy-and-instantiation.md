# RFC 0021: Component hierarchy and deterministic instantiation

- Status: Accepted; bounded local-source v1 implemented and verified
- Authors: Eqiora contributors
- Created: 2026-07-18

## Summary

A component is a typed source declaration whose instances deterministically
expand into the same flat canonical Relation network as equivalent explicit
source; hierarchy is never a second model truth or a runtime object graph.

## Motivation

Reusable motors, controllers, thermal elements, and block-diagram fragments
need names, interfaces, parameters, private internals, and nested instances.
Those concepts are larger than packaging. They change source scoping and model
elaboration even when every definition is local to one file.

Putting hierarchy directly into the Semantic Kernel would make component
identity compete with Domain, Field, Relation, Activation, and Connection for
canonical authority. Treating components as textual macros would preserve a
flat graph but lose typed interfaces, stable diagnostics, and instance
identity. Packaging cannot choose between those semantics as a side effect of
dependency resolution.

This RFC therefore defines hierarchy first. Package identity and distribution
are a separate contract in RFC 0022 and depend on this elaboration boundary.

## Proposed design

### One semantic endpoint

Compilation has an explicit elaboration stage:

```text
typed source declarations
        |
        v
definition and instance graph
        |
        | deterministic elaboration
        v
flat canonical Relation network + provenance map
```

The definition and instance graph is compiler-owned input to elaboration. It
is not a Semantic Kernel graph, is not executable, and does not survive as an
independent mutable model. The resulting canonical model uses the existing
kernel node kinds and connection semantics.

A component definition may contain:

- typed public parameters and ports;
- private parameters, fields, relations, activations, and local declarations;
- typed signal or conserving connector interfaces;
- nested component instances; and
- explicit connections between visible interfaces.

An instance is a compile-time binding of one definition at one lexical path.
It is not a runtime allocation or a canonical node kind.

### First source slice

The first implementation uses compilation-unit-scoped connector and component
declarations followed by ordinary model declarations. Its closed syntax is
source-shaped as follows:

```text
connector Pin = scalar_physical(
  across = kg * m ^ 2 / (s ^ 3 * A),
  through = A
);

component Resistor {
  public parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2);
  public port positive: conserving on Pin;
  public port negative: conserving on Pin;
  relation law continuous { ... }
}

model parallel {
  instance r2: Resistor(resistance = 2);
  instance r4: Resistor(resistance = 4);
  connect conserving r2.positive, r4.positive;
}
```

Component-body declarations are private by default. In v1, only scalar
Parameters and Ports may be marked `public`; Relations, Fields, Activations,
local Connections, and nested instances remain implementation details.
Instances may occur in a model or component body. A qualified member selection
may name only a public Port, and parameter bindings may target only public
Parameters. Public Parameters are configured at instantiation rather than
mutated through member selection.

A `connector` declaration is a nominal connector family, not a Kernel node.
The first family is scalar physical. One resolved connector declaration
elaborates to one canonical scalar-physical Domain in the root model, shared by
all Ports instantiated from that declaration. Equal dimensions from different
connector declarations remain incompatible. Existing component-local signal
Ports use the current explicit `signal input` or `signal output` syntax; a
reusable signal connector-family declaration is deferred.

Bindings are pure compile-time scalar expressions. V1 admits numeric literals,
constants, and enclosing public Parameters with ordinary arithmetic. A bare
literal is interpreted in the target Parameter's declared coherent-SI
dimension, matching an ordinary Parameter initializer. Fields, Ports, time,
derivatives, `pre`, `next`, and function callbacks are forbidden in bindings.
The complete binding dependency graph is dimension-checked and proven acyclic
before elaboration.

### Names and deterministic identity

Every definition has a fully qualified declaration name. Every instance has
an `InstancePath` formed from the root instance followed by source-declared
instance names. Every elaborated declaration is identified by:

```text
InstancePath + definition-relative declaration path + declaration kind
```

The encoding is length-delimited and versioned; it never depends on display
punctuation, map iteration order, source file order, import order, allocation
order, or compiler traversal order. Canonical collections sort by encoded
identity. Duplicate names in one scope fail, while equal local names under two
different instance paths remain distinct.

In v1 the root path begins with the model declaration name. Each nested segment
is the exact source-declared instance name after lexical resolution; display
aliases and source-file names never become path segments.

Compiler-generated declarations use a reserved namespace and an operation
index derived from canonical structural position, not a process-global
counter. User source cannot spell that namespace.

The concrete v1 identity preimage is an `ElaborationKey` containing a namespace,
an `InstancePath`, a definition-relative declaration path, and either an entity
kind or a reserved generated role. Each UTF-8 segment is encoded with an
explicit schema tag and checked big-endian length; no display punctuation is
identity. The namespace is the canonical typed source-unit identity for local
compilation and the exact Model Package identity for package compilation.

Eqiora retains the complete SHA-256 key digest. Existing Kernel identifiers
use a deterministic 128-bit projection of that digest through `Id::from_ulid`.
The staging allocator detects any projected collision between unequal full
digests before it emits a Transaction and fails closed. Generated Activations
use a reserved role. Anonymous Connections first sort exact member identities,
then derive their canonical structural position. Legacy flat source keeps its
current identity behavior; this seam applies only to elaborated declarations.

### Parameters and visibility

Bindings target only declared public parameters. A binding is checked in the
definition's lexical environment before elaboration and must preserve value
kind, scalar type, shape, and physical dimension. Unbound required parameters,
unknown names, duplicate bindings, and dependency cycles fail before any graph
transaction is committed.

Private declarations can be referenced by their owning definition and nested
definitions according to lexical scope, but cannot be selected through an
instance's public interface. Provenance may name a private declaration for a
diagnostic without making it public model API.

### Connectors and connections

Signal and conserving interfaces remain different types. Aggregation may give
either interface named members, but a record or bus shape does not acquire
causality, conservation, activation order, or scheduling semantics merely by
being aggregated.

Elaboration checks connector kind and member compatibility before producing
canonical Connections and Relations. A signal interface may express declared
causal direction. A conserving interface elaborates through the existing
acausal connection contract. No implicit conversion exists between them.

Expandable connectors and stream semantics are outside this RFC; accepting an
unknown member or inventing a connection equation is an error.

### Atomic elaboration and provenance

Name resolution, binding, connector checking, recursion detection, size
calculation, and identity construction complete in a bounded staging area.
Only a fully valid expansion emits one graph transaction. Failure leaves the
base revision unchanged.

The compiler emits a sidecar provenance map from every elaborated identity to
its definition span, instance span, and binding spans. Diagnostics can present
the instance stack without making source locations part of canonical model
identity. Formatting or file relocation may therefore change source
provenance while preserving semantic bytes.

The first sidecar is immutable compiler output and has no durable public wire.
A later versioned provenance envelope may preserve the same mapping for Studio
or cross-package diagnostics without changing the Semantic Model or Model wire.

### Bounded recursion

The definition dependency graph must be acyclic in v1. Direct and indirect
recursive instantiation fail with the complete cycle. Before expansion, the
compiler enforces configured limits for nesting depth, instance count,
declaration count, connection count, identifier bytes, and total source bytes.
Count and size arithmetic is checked before allocation.

### Canonical equivalence

Hierarchy is syntactic and organizational sugar over canonical meaning. Given
the same resolved declarations and bindings, a component instance and an
equivalent explicit model must produce the same canonical Relation network
after canonical identity normalization. Layout, icons, source grouping, and
instance display labels stay in projection or provenance data.

## Prior art and deliberate differences

The Modelica language specification supplies useful prior art for qualified
packages, lexical encapsulation, import-order independence, flattening, and
typed connection equations:

- [Scoping, name lookup, and flattening](https://specification.modelica.org/maint/3.7/scoping-name-lookup-and-flattening.html)
- [Connectors and connections](https://specification.modelica.org/maint/3.7/connectors-and-connections.html)
- [Packages](https://specification.modelica.org/maint/3.7/packages.html)

Eqiora borrows the idea that hierarchy elaborates to equations, not a runtime
component heap. It does not adopt Modelica inheritance, modification and
redeclaration, tool-dependent library search, expandable connectors, stream
connectors, or semantic behavior hidden in annotations. Each can be proposed
later only with its own canonical contract and falsifying evidence.

## Alternatives considered

### Preserve hierarchy in the Semantic Kernel

This makes source navigation direct and incremental edits convenient, but it
adds component and instance authority to mathematical meaning and requires
every executor to interpret hierarchy. Rejected as the canonical form. Studio
may retain a projection indexed by the provenance map.

### Pure textual expansion

This has a small implementation surface but makes capture avoidance, typed
interfaces, private visibility, and diagnostic instance stacks accidental.
Rejected.

### Typed source IR followed by deterministic flattening

This keeps one semantic endpoint while making hierarchy inspectable and
testable before mutation. Adopted.

## Compatibility and migration

The first implementation is additive source syntax and compiler IR. It does
not add Semantic Kernel node kinds or change existing explicit models. A
versioned provenance sidecar may be added to compile artifacts independently
of canonical model wire versions.

Existing reusable helpers remain built in until an ordinary component path
matches their diagnostics, canonical output, and verification. Migration is
case-by-case; the RFC does not authorize two permanent lowering paths.

## Verification

1. Instantiate one definition twice and prove distinct, stable identities with
   identical local structure.
2. Permute files, imports, declarations that are semantically unordered, and
   internal map insertion; canonical bytes must not change.
3. Compare an instance with an equivalent explicit model after identity
   normalization; relations, activations, and connections must match.
4. Reject missing, duplicate, unknown, wrong-shape, wrong-scalar, or
   dimension-incompatible parameter bindings before graph mutation.
5. Reject signal/conserving interchange and incompatible aggregate members.
6. Prove that private declarations cannot be selected through a public
   instance interface.
7. Reject direct recursion, indirect recursion, excessive depth, excessive
   counts, and identifier-size overflow without a partial revision.
8. Preserve definition, instance, and binding spans in diagnostics while
   showing that formatting-only changes do not alter canonical semantics.
9. Elaborate an acausal DC motor and a discrete causal controller into one
   hybrid Relation network without example-specific kernel nodes.

The bounded implementation covers compilation-unit Connector and Component
definitions, public scalar Parameter and Port interfaces, nested local
instances, deterministic collision-checked identity, atomic resource-bounded
expansion, and an immutable in-memory provenance sidecar. The registered
[`language.component-elaboration`](../verify/language/component-elaboration/README.md)
case instantiates one component definition twice inside a closed nested
physical system, proves semantic permutation and source relocation do not
change model identity, compares the physical result with explicit flat source,
solves the analytic circuit, and exercises fail-closed scalar bindings,
visibility, nominal connectors, and recursion. It closes the bounded
local-source portions of items 1--4, 6, and 8. Compiler-level tests cover
configured depth, staging, provenance, and identity limits.

The registered
[`hybrid.packaged-dc-motor-controller`](../verify/hybrid/packaged-dc-motor-controller/README.md)
case now closes item 9 for one exact three-package scalar ideal-linear slice.
It also proves item 5 for nominal electrical/rotational mismatch and
signal/conserving interchange. Its reference-only transient and component
scope is governed by RFC 0027 and does not widen the remaining nonclaims here.

Import and multi-file permutation require RFC 0022 package resolution. Wider
binding shapes and scalar types, structured or aggregate connector checks,
complete resource-limit evidence, durable diagnostic presentation, and the
hybrid component breadth remains open. Item 7 is split between registered
recursion rejection and compiler-only resource-limit regressions.

## Security, safety, and governance

Elaboration consumes untrusted source and package content. It performs no
native loading, build scripting, network access, or backend discovery. All
limits are checked before graph mutation, and diagnostics bound cycle and
instance-stack rendering. A future change that preserves hierarchy as
canonical meaning, admits recursive expansion, or invents connector members
requires a new RFC.

## Nonclaims

This RFC does not define package distribution, dependency versions, imports,
inheritance, traits, replaceable or redeclared components, arrayed instances,
expandable or stream connectors, cross-boundary connection-set union,
inside/outside signs, dynamic hierarchy, statechart hierarchy, layout
inheritance, icons, model references, or runtime scheduling. Native, Python,
and Studio hierarchy authoring, a durable provenance wire, and nonlinear,
transient, distributed, or accelerator execution remain separate contracts.

## Deferred questions

The accepted v1 boundary above fixes source syntax, constant bindings, identity,
and an in-memory provenance sidecar. A durable provenance wire, reusable signal
connector families, structured Parameters, arrayed instances, and every item in
the nonclaims require separate evidence and, where compatibility changes, a
follow-up RFC.
