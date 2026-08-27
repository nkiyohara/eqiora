# Studio run lifecycle status

The former Studio-controlled `ReferenceRunPlan`/`ReferenceRunOutcome` workflow
has been retired. Studio no longer owns a parallel application-shaped solver
lifecycle, and this page records no executable capability or compatibility
promise for those removed types.

The retained product lifecycle is Model-first and physics-neutral:

```text
.eqi -> compile(Geometry) -> resolve(Model, Mesh, typed policies) -> Plan
     -> run(Plan) / submit(Plan) -> Result or State
```

Studio presentation may project artifacts produced through that root lifecycle,
but it must not infer physics, rebuild numerical policy, or introduce a second
Plan/Run identity. A future Studio execution surface therefore needs fresh,
claim-local evidence against the common lifecycle rather than revival of the
retired reference-run API.
