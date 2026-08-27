# Eqiora Studio

Eqiora Studio is an accessible desktop projection of canonical Eqiora models.
It is a client of the public Rust facade, not a second model implementation.

The current Studio surface contains:

- Eqiora source compile/check, semantic outline, relation view, inspector, and
  source-linked diagnostics;
- coherent-SI `Field` and `Parameter` value-edit preview, atomic commit, and a
  bounded immutable revision lineage;
- workspace-only graph layout and keyboard-accessible commands;
- the verified packaged DC-drive presentation, using its existing pinned
  package closure and accepted sampled trajectory;
- exact bounded CAD replay and Domain selection; and
- authored-CAD construction, replay, inspection, and export.

Studio does not currently expose scalar PDE, exact-cylinder flow, structural,
FSI, or generic ODE Plan/State/Run workflows. Those application-shaped
lifecycles were retired instead of being adapted around the common Model-first
API without a truthful caller-owned Geometry path.

## Boundaries

```text
React presentation
      ↓ runtime-validated Studio DTO
Tauri command adapter
      ↓ public Eqiora facade
canonical compiler / value transaction / CAD / packaged DC owner
```

The bridge is `eqiora.studio.bridge/v5`; it is independent of the canonical
Model wire. The browser development view is explicitly a preview: it does not
parse or execute Eqiora semantics and never fabricates scientific results.

`src/application-registry.ts` owns the closed presentation registry.
Applicability is derived from typed accepted state, never source-text or
component-tree inspection. CAD remains bound to the exact current Model
digest. The packaged DC command is a presentation composition over its existing
scientific owner; React derives no physical quantities.

The Tauri shell has no filesystem, shell, or remote-content permission.

## Develop and verify

Use the repository-owned Node version and commands:

```bash
npm ci
npm run check
npm test
npm run build
```

Rust formatting and checks are owned by the workspace tasks described in
`docs/development/local-verification.md`.
