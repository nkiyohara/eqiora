# Reference construction

The physical values come from two consecutive executions of the finalized CPU
operator registered by `fsi.fixed-reference-monolithic-step-2d`. The second
step consumes the first accepted velocity and displacement state. The input
state is not published as `t0` because it has no accepted algebraic pressure;
inventing zero pressure would create a false complete observation.

Logical numerical leaves are canonical `DiscreteFieldEnvelopeV1` artifacts in
whole-mesh entity order. Semantic support is carried by
`FieldSnapshotEnvelopeV1`: coefficients outside the support closure are
positive zero. The fluid MINI velocity is the ordered pair of one Vertex leaf
and one Cell leaf.

Trajectory publication is a content-addressed DAG:

```text
discrete Field -> Field snapshot -> spatial state -> segment -> trajectory
                                                             -> Dataset view
completed Run ----------------------------------------------> output reference
```

Storage chunk manifests are optional witnesses over canonical discrete-Field
bytes. They never enter snapshot, state, trajectory, or Dataset identity.
