# Observation reference

The oracle observes only the current public facade:

- `eqiora::language::{parse, format, Document, Item, RelationDecl, Expr,
  ExprKind, BinaryOp, UnaryOp, TextRange}` for ordered trees and parser-owned
  ranges;
- `eqiora::api::ModelDocument` compilation, native definition, public
  structural comparison, fingerprint, and the bounded fresh-identity guards;
- the public package authoring, preparation, in-memory store, exact resolution,
  and locked compilation sequence; and
- public diagnostic code, message, graph-path, and optional source-span
  accessors.

Private `Tree`, `RangedTree`, `DiagnosticClass`, `Observation`, and fixed
mutant records are test-local assertion vocabulary, not public APIs or durable
schemas. No implementation field, source map, parser/compiler sidecar, exact
producer replay, snapshot, generated output, or second registry is used.

Structural equivalence is intentionally narrower than algebraic or numerical
equivalence and intentionally independent of occurrence-bearing artifact
identity. Package and native probes retain their own exact lineage and are
compared only for the same ordered structural/static meaning.
