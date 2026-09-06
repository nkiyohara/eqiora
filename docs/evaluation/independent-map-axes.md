# Independent evaluation axes

This contract and executable indexing reference specifies independent maps.
It does not expose an installed map or make another numerical
profile executable. The reference is private, native, and test-only, beside the existing
Study owner in [`axes_reference.rs`](../../crates/eqiora-api/src/parameter_study/axes_reference.rs).
It admits and indexes dense buffers without an evaluator, callback registry, or solver.

## Existing authorities

A map retains one exact program signature. The current
[`DifferentiableProgramIdentity`](../../crates/eqiora-api/src/differentiation.rs) owns Model,
Plan, ordered Parameter inputs, selected output Field, flat dimensions, scalar precision,
device, derivative contract, and solver. `DifferentiableParameterPoint` owns the complete
ordered input point. An accepted `DifferentiableEvaluation` owns its primal and linearization;
mapping cannot create, replace, or infer that acceptance.

Mathematical dimensions, real/complex domain, component shape and frame come from the existing
`ParameterDef`/`FieldDef` and `ValueType`. Support, activation and Representation stay on the
semantic graph; the exact Plan retains Mesh, spaces, placement and numerical layout. Output
Field and coefficient associations remain with the accepted Result. Retain these owners,
including any admitted basis identity, rather than creating a batch copy of their metadata.
Absent metadata or an unsupported signature rejects; an equal flat length is insufficient.
The current differentiable program admits real binary64, host-CPU scalar Q1/TPFA output only.
The indexing reference can retain a complex `ValueType`, but performs no complex evaluation.

A batch axis is a collection axis outside this mathematical type. In particular,
`ValueType::array` describes a component array and cannot represent an empty batch. Batch
axes do not change field support, clocks, physical interactions or component bases.

## Axes and composition

Each input leaf explicitly chooses one mapped axis or sharing at each map level. All mapped
axes at one level have the same extent; neither singleton broadcasting nor truncation is
implicit. A shared value is immutable and is used unchanged at that level. If every input
is shared, the extent must be explicit. An explicit extent must also agree with mapped inputs.

Input positions refer to axes in the complete dense buffer. Removing those axes, while
preserving the order of the remaining axes, must recover the exact unbatched leaf shape.
For example, two channels mapped at position 1 have buffer shape `[2, N]`; the two channels
remain one member's components. One physical axis cannot serve two map levels.

Outputs have a static ordered leaf structure supplied by the signature before execution.
Each output declares where every map axis is inserted into its unbatched shape. An output
cannot omit a map axis merely because values happen to agree. Within this reference, buffers
are contiguous row-major binary64, and axis insertion changes indexing, not element meaning.
Other layouts require explicit admission by the existing execution owner.

At one level, multiple mapped inputs are zipped. A Cartesian product is an explicit nested
map: in the outer level map input `a` and share `b`; in the inner level share `a` and map `b`.
Occurrence coordinates are ordered outermost to innermost; the final coordinate varies fastest.
Two-level buffers name both positions in the complete shape, so removing an outer axis cannot
silently shift the meaning of an inner position. Nested mapping introduces no communication,
collective, recurrence, or interacting physical component.

## Identity and acceptance

The occurrence is the request's positional coordinate, not its numerical point. Preserve
duplicates, signed-zero bits, and requested order. Permuting inputs and attached sample IDs
permutes outputs and their associations; the new request positions describe that new order.
Do not sort or deduplicate, and do not require a default-point anchor.

An optional stable sample ID selects stochastic replay only through an admitted sampling
owner. Repeating it can intentionally request the same path; independent replicates use
distinct IDs. It is neither the occurrence coordinate, the exact input point, nor an accepted
evaluation identity. Equal IDs do not authorize reuse at a different point or under a foreign
sampling law, Plan or environment. The reference only carries caller-supplied IDs alongside
indices; it does not generate paths or invent accepted-evaluation IDs.

Dense admission requires the same exact program/Plan signature for all members, including Mesh
identity and output structure/association. A different mesh with the same coefficient count,
ragged leaf lengths, different leaf structure, or foreign output ownership rejects. Explicit
grouping or a heterogeneous collection needs its own admitted interface; padding is not a map
operation. The reference checks dense buffer shape and retains an actual program identity in
its signature tests; installed member acceptance remains the existing evaluation owner's job.

## Empty maps and resource admission

Zero and singleton extents are admitted. The output signature, leaf types and shapes exist
before a first member runs, so a zero extent returns correctly typed empty buffers and performs
zero evaluations. A mean or another reduction with a nonempty domain must reject an empty
collection separately. Sharing a value over an empty extent does not remove its structural
validation. There is no mandatory anchor or two-member minimum in this contract.

Before allocating member/output buffers, check rank, axis positions, duplicate axes, all shape
products, addressable strides, input lengths, total output values and estimated bytes against
explicit resource limits. A zero factor does not excuse overflow in other logical strides.
The small reference bounds rank and retained binary64 values/bytes explicitly; these are
caller-provided admission limits, not a permanent general-map maximum. They estimate these
buffers only, not total solver memory. A solve-map consumer must add its own admitted per-member
state/history and execution resource estimate before allocation.

## Executable reference and delivery boundary

The tests use `f(a, b) = (2*a + b, a - 3*b)` with exactly representable integer values.
For zipped `(a,b) = [(2,10), (1,20), (2,10)]`, the expected two output leaves are
`[14,22,14]` and `[-28,-59,-28]`. Sharing `b=10` gives `[14,12,14]` and `[-28,-29,-28]`.
The product of `a=[1,2]` and `b=[10,20]` gives, in outer-`a` order,
`[12,22,14,24]` and `[-29,-59,-28,-58]`. These hand-derived values distinguish zip,
product, ordering, duplicates and accidental sharing. Non-leading component axes, nested
axes, static empty outputs, shape/axis/extent errors, aggregate budgets, metadata preservation,
sample association and foreign program signatures have separate focused tests.

Run the repository-owned gate with `mise run fast`; the new reference is included in the
`eqiora-api` unit tests. This introduces no registered evidence or scientific verification.
The current sorted, unique, default-anchored `ParameterStudyPlan` still implements its existing
narrow behavior: it is not an implementation of this map contract. Its replacement with ordered
accepted evaluations requires an atomic consumer and evidence migration in a subsequent slice.
Map AD, frontend adapters, stochastic execution, threading and persistence remain subsequent
capabilities. No new source-language executor or source syntax is introduced here.
