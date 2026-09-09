# Typed Observable identity

- Status: Accepted
- Implements: #882

An Observable retains one typed expression and optional exact spatial reduction in
Model meaning. It adds neither a solve unknown nor an equation. Its result is
selected and evaluated against an accepted Result's exact Model and execution lineage.

Add the semantic kernel kind `Observable`. A Field owns an unknown or evolving
state; using it would require every solve inventory to rediscover that some Fields
are derived. A Relation owns equations and activation. A Parameter owns a static
value. None retains a derived expression with this invariant. The evidence-graph
Observation records an experiment observation and does not own Model meaning.

`ObservableDef` owns a complete result type, one expression DAG root and either a
value or spatial integral over an exact Domain with volume or boundary measure.
Whole-Model admission infers expression types and checks support and resulting
units. Boundary orientation uses the existing Domain and normal-component operator;
quadrature and reconstruction remain numerical choices. Private component members
remain inspectable by their exact qualified identity without becoming public ports.

This pre-1.0 addition migrates the current semantic artifact and source projections
atomically. Historical release artifacts remain historical. Complex, partial-domain,
stochastic and trajectory reductions require their own admitted contracts; this
entity does not claim their execution or differentiation automatically.
