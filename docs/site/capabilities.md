# Capabilities

## Start with what runs

The full matrix opens with a concise, plain-language
[Executable problem classes](https://github.com/nkiyohara/eqiora/blob/main/docs/capability-matrix.md#executable-problem-classes)
table. It answers “what kinds of problems have a complete path today?” and
links every row to its evidence.

Use that table in three steps:

1. Find the closest problem class.
2. Open its evidence to check the supported inputs and assumptions.
3. Return to the topic-specific matrix only when you need contract, execution,
   verification, or maturity detail.

The repository matrix is the single maintained orientation. This site links to
it rather than copying scientific claims that could drift.

## Why the matrix has four gates

Eqiora reports capability maturity along four independent gates:

| Gate | Question |
|---|---|
| Contract | Is there a typed, versioned semantic or lowered contract? |
| Execution | Does at least one real end-to-end path execute? |
| Verification | Does reproducible evidence support the exact stated scope? |
| Maturity | Is the capability broad, robust, documented, and suitable for general users? |

The complete, maintained product map is the
[capability and maturity matrix](https://github.com/nkiyohara/eqiora/blob/main/docs/capability-matrix.md).
It covers semantics, numerics, physics, execution, artifacts, Python, Studio,
and OSS maturity.

## Trace a claim to evidence

The [evidence catalog](evidence/index.md) is generated during the site build
from `eqiora-verify index --format json`. It links stable capability identifiers
to validated case manifests without re-declaring their scientific claims.

Case manifests and their referenced evidence remain authoritative. A generated
entry means the manifest was accepted by the repository index contract; its
status states whether the case is proposed, specified, implemented, verified,
or validated.
