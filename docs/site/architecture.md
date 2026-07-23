# Architecture

Eqiora is organized around one directional path:

```text
semantic meaning
      ↓
lowered contract
      ↓
realization
      ↓
adapter / execution provider
      ↓
artifact and evidence
```

This direction prevents authoring clients, numerical methods, storage formats,
or hardware adapters from becoming alternate sources of model meaning.

## Two layers, several projections

The semantic model owns canonical relations, activation, typed ports,
connections, and identity. The realization owns discretization and execution
choices. Block diagrams, state charts, physical networks, field views, Python,
and Studio are projections or clients of those shared contracts.

Artifacts bind exact model, realization, execution, and result lineage without
turning provenance into model semantics. Verification manifests separately bind
bounded claims to reproducible evidence.

## Authoritative design material

- [Architecture summary](https://github.com/nkiyohara/eqiora/blob/main/docs/architecture.md)
- [Glossary](https://github.com/nkiyohara/eqiora/blob/main/docs/glossary.md)
- [RFC process and records](https://github.com/nkiyohara/eqiora/tree/main/rfcs)
- [Library and accelerator strategy](https://github.com/nkiyohara/eqiora/blob/main/docs/development/library-and-accelerator-strategy.md)
- [Language baselines](https://github.com/nkiyohara/eqiora/blob/main/docs/development/language-baselines.md)

These repository documents, not this overview, define the detailed contracts.
