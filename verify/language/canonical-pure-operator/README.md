# Canonical pure operator

This case verifies the first source-declared consumer of Eqiora's bounded,
capture-free component calculus. The fixture declares a public dyadic product
of two spatial vectors and calls it both locally and through an exact package
alias. Lowering emits one generic `PureOperatorApplication` whose dispatch key
is the content digest of its closed definition; the source name `dyadic`, file
path, formal names, declaration order, and package alias are not dispatch.

Direct source permutations produce identical current Model bytes. Two exact
operator releases with relocated, reordered, and formal-renamed source have
one package semantic identity, while two root releases using different exact
dependency aliases also have one root semantic identity and identical Model
bytes. The local and package-resolved expressions retain the same
definition digest.

The same compiled definition scalarizes pointwise vectors `[2, 3]` and
`[5, 7]` in row-major order to:

```text
[10, 14, 15, 21]
```

The current Model and Transaction replay with exact canonical bytes. Mutated
current documents with an unknown required feature, a forged definition
digest, or a definition count over the selected decoder limit fail before
graph mutation. The lower-level artifact regression
suite separately exhausts missing, duplicate, unused, forward-reference,
arity, and aggregate Relation/Activation-guard bounds.

Run:

```bash
cargo test --locked -p eqiora --test canonical_pure_operator
cargo run -p eqiora-verify -- run --case language.canonical-pure-operator
```

This case does not claim general contraction, broadcasting, reduction, weak
forms, support transfer, numerical discretization, solver or backend
selection, floating-point reassociation, callbacks, or dynamic plugins.
