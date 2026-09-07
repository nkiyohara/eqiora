# Independent evaluation axes

This contract specifies independent maps. A native ordered map now executes complete input
points through the existing Q1/TPFA differentiable programs. Their accepted JVP/VJP products
support nested point/seed grids using the same private axis owner as the arithmetic reference,
[`axes.rs`](../../crates/eqiora-api/src/evaluation_map/axes.rs).
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

The indexing reference remains in `eqiora-api` unit tests. Native `EvaluationMapPlan` accepts
one shared `Arc<DifferentiableProgram>`, a request-ordered slice of complete input slices and
an explicit retained-numerical-byte limit. Its `execute` returns either `CompleteEvaluationMap`
or `EvaluationMapTerminalReport`. `members()` and `evaluation(index)` on a complete result
preserve every position. A report's `occurrence(index)` distinguishes accepted, failed,
cancelled and not-started positions; its accepted prefix remains individually inspectable.
Cancellation is polled only before/between members. Empty completion never polls, final
completion wins, and a numerical failure inside evaluation precedes a newly raised cancellation.

For an already compiled program with three selected inputs in the declared order:

```rust
use std::sync::Arc;
use eqiora::api::EvaluationMapPlan;

let program = Arc::new(program);
let p1 = [2.0, 0.75, 0.5];
let p2 = [3.0, 1.25, -0.25];
let plan = EvaluationMapPlan::new(program, &[&p2, &p1, &p2], 64 * 1024 * 1024)?;
match plan.execute() {
    Ok(complete) => assert_eq!(complete.members().len(), 3),
    Err(report) => eprintln!("Stopped at {}: {:?}", report.stopped_index(), report.diagnostics()),
}
```

The native storage estimate includes complete point/member records, state/RHS vectors, a
conservative dense upper bound on CSR nonzeros, output and residual Parameter Jacobians, and
output-state associations. It checks products before copying input points or reserving output
members. It excludes the already shared Program, deployment metadata, allocator overhead,
diagnostics and transient solver workspace; it is not a peak-process-memory bound. The
existing accepted Program owns the fixed state/output dimensions; no default sparsity count
is assumed to remain unchanged at a new point.

The old single-Parameter sorted/unique/default-anchored Study API has been removed. Its two
registered composition cases are migrated in place to ordered complete points and partial
accepted-member inspection, using unchanged independent pointwise evaluation as the reference:

```bash
mise run affected -- \
  --case differentiation.bounded-parameter-study \
  --case differentiation.bounded-parameter-study-private
```

## Accepted first-order products

`EvaluationMapProducts` borrows a `CompleteEvaluationMap`; a terminal report is not an
admissible derivative collection. The shared Parameter IDs are selected explicitly in the
exact Program coordinate domain. Their values must be identical at every occurrence; equal
values never imply sharing. Remaining Parameters are mapped in original Program order.

The product view separately admits `point_shape`, `seed_shape` and the positions of point
axes in their combined grid. Point shapes flatten to the original request inventory; seed
shapes enumerate independent first-order products. For example, point `[3]`, seed `[2]`,
and point-axis position `[1]` give seed-major `[2,3]`; `[0]` gives point-major `[3,2]`.
More than one point or seed axis can be nested and interleaved. These are record axes, not
physical Field component axes or derivative order. Numerical coordinates remain real
coherent-SI values in the admitted Program's input/output pairing.

```rust
use eqiora::api::EvaluationMapProducts;

// `complete` has three accepted points sharing this exact input coordinate.
let shared = [complete.plan().program_identity().inputs()[0]];
let products = EvaluationMapProducts::new(&complete, &shared, &[3], &[2], &[1], 1 << 30)?;
// Shared tangents: seed axes then shared coordinates.
// Mapped tangents: combined point/seed axes then remaining coordinates.
let jvp = products.jvp(&shared_tangents, &mapped_tangents)?;
let vjp = products.vjp(&output_cotangents)?;
assert_eq!(jvp.products().len(), 6);
```

For `y_i=f(s,x_i)`, JVP assembles `Ds_i ds + Dx_i dx_i`. VJP preserves each occurrence's
mapped covector and **sums** shared contributions over points, separately for every seed.
The original request order determines floating-point accumulation, independently of axis
placement. Overflow is an error; arbitrary regrouping is not promised bit-identical.
An arithmetic mean requires a separate explicit outer reduction.

Each result retains the unchanged ordinary JVP/VJP record, complete primal output and exact
point/Plan evidence. Neither execution nor reverse execution reruns the primal. Malformed or
nonfinite inputs reject before actions; an action, evidence or finite-sum failure publishes
no partial derivative collection. Empty point axes produce empty records and zero shared
sums; zero seed axes perform no derivative actions. Only real first-order implicit products
exist: product results cannot be differentiated again, and complex/profile adapters remain
separate admissions.

An explicit byte limit bounds numerical buffers for one complete JVP and one complete VJP,
including their cloned evidence-point coordinates, duplicate primal payloads, full/mapped
covectors and shared sums. It does not bound already retained maps, other metadata/record
allocations, caller buffers, solver scratch or peak process memory. The
shared private axis owner checks ranks, extents, strides and products before allocation.
The [mapped-product case](../../verify/differentiation/mapped-products/README.md) derives the
shared chain rule independently and compares current Q1/TPFA products with ordinary retained
evaluation actions. Run it with `--case differentiation.mapped-products` in addition to the
two ordered-map cases above when changing this composition.

This native map consumes explicit complete points. General installed input-axis adapters,
Python/JAX/PyTorch batching, higher derivatives, stochastic execution, threading and
persistence remain subsequent capabilities. No new source-language executor or source
syntax is introduced here.
