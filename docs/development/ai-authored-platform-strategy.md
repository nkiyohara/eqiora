# AI-authored platform strategy

- Status: Accepted
- Created: 2026-07-25
- Baseline revision: `f5ae8c5`
- Related: [library and accelerator strategy](library-and-accelerator-strategy.md),
  [vertical-slice development](vertical-slice-development.md),
  [roadmap](../roadmap.md)

## Purpose and authority

This document owns one decision: **what Eqiora optimizes for when its code is
written by AI agents rather than a human team**, and which existing rules that
premise changes.

It does not restate the accelerator strategy, redefine canonical meaning, or
duplicate the roadmap's dependency order. Where it amends an existing rule, the
amendment is listed here and applied in the owning document; this file is not a
second registry.

## Premise

Eqiora is authored by AI agents. The scarce resource is therefore not
implementation throughput.

> The binding constraint is whether an agent can **mechanically establish that
> what it wrote is correct**.

Three consequences follow, and they set every priority below.

1. **Bulk hand-written physics is the anti-pattern.** Not because writing it is
   slow — it is not — but because each module carries an independent oracle, an
   independent bug surface, and no shared proof. Review cost, not authoring
   cost, scales with it.
2. **Fail-closed discipline is the enabling substrate, not overhead.** An agent
   can safely emit a *form*. An agent cannot safely hand-write a multi-thousand
   line physics module with its own bespoke conformance harness. The existing
   bounded-claim / independent-falsifier / registered-evidence rules are what
   make agent authorship survivable, and they are retained without weakening.
3. **Beauty is a checked predicate, not a taste.** A codebase that only humans
   can judge as clean cannot be maintained by agents. Structural quality moves
   into CI or it does not exist.

## Position

Eqiora is:

> the CAE platform whose correctness is machine-checkable end to end — so that
> agents can extend the physics safely, and every result carries its own
> evidence.

Eqiora is **not** pursuing fastest-in-class execution. That seat is occupied,
costs roughly linear headcount to contest, and is orthogonal to the property
above. Performance claims remain workload-specific and must fix hardware, mesh,
ordering, solver and preconditioner, tolerances, transfer accounting, warmup,
and repetitions, per the accelerator strategy's evidence rules. No global
speed ratio against another framework is adopted as a target; such a rule is
underspecified and currently unsupported by any registered measurement.

## Measured baseline

Independently measured at `f5ae8c5` by both reviewing agents; figures agreed.

| Observation | Value |
| --- | --- |
| `eqiora-numerics` | 63,034 lines, 122 `src/` files — largest crate |
| Repeated role skeleton across independent physics owners | 22 files, 11,178 lines |
| — `api.rs` / `assembly.rs` / `acceptance.rs` / `element.rs` / `newton.rs` | ×7 / ×4 / ×4 / ×4 / ×3 |
| `src/` files over 1,500 lines | 30 |
| `src/` files over 1,000 lines | 57 |
| AST-reachable public items, `eqiora-numerics` / `eqiora-api` | 308 / 124 |
| Glob re-exports | 37 across 4 files, 26 of them in the `eqiora` facade |
| Krylov methods | CG, MINRES, BiCGSTAB |
| Preconditioners | Identity, Jacobi |
| CUDA backend | cuSPARSE/cuBLAS adapters over a finalized CSR; no custom kernels |

The 11,178-line figure is the **candidate surface area of repeated roles**. It
is not yet established that those lines are clones; the architecture scanner
below is what will decide that. Earlier framings claiming that most of the
crate is duplicated proof machinery are withdrawn as unsupported.

## The first lane: an FEM form compiler

### Why this and not more physics

Every arrow in the roadmap's physics chain is a hand-written vertical slice.
Under the premise above, that chain encodes the wrong growth curve: each new
physics re-earns its proof machinery instead of inheriting it. The lane's
purpose is therefore **amortizing evidence**, not adding physics faster.

### Scope, stated narrowly

This is an **FEM** form-compiler lane. A universal weak-form IR is rejected on
its merits, not by precedent: conservative finite-volume face fluxes are
method-foreign to a variational form and must not be routed through one. The
FVM path keeps its existing evidence and is explicitly outside this lane.

The first slice does not introduce a new language, a public form IR, or a new
crate. It introduces **one private, proof-carrying FEM derivation and evaluator
for an already recognized strong-form subset**.

The `.eqi` surface language is not sufficient input on its own. Relations are
componentwise *strong* residuals; there are no test or trial functions,
measures, integrals, element families, quadrature, DOF layouts, or weak
boundary-term disposition, and field declarations do not carry unknown versus
coefficient or design roles. Supplying that structure — fail-closed — is part
of the work, not a precondition assumed to exist.

### Two-state evidence model

This is the load-bearing definition for the whole lane.

| State | Meaning |
| --- | --- |
| **Compiler-admissible** | Executable within one explicitly bounded compiler subset. Not a capability claim. |
| **Verified capability** | Independently established by a registered case. Only this justifies a `verified` row in the capability matrix. |

Admission requires all of the following, failing before assembly: replayed
typed static semantics; closed role assignment where every semantic node is
consumed exactly once and ambiguity fails; a derivation certificate binding
versioned rule IDs to exact source relation identities; realization
compatibility naming the exact spaces, reference cell, DOF ordering, geometry
map, and quadrature policy; and bounded compilation limits on DAG size,
derivative order, quadrature points, local DOFs, and generated work.

Verification additionally requires an independent element oracle, derivative
falsification against independently rebuilt finite differences, assembly and
action equivalence against independent CSR assembly, a per-instance numerical
witness, a mutant set each rejected by a *named* gate, and a registered case.

The division that makes this pay off:

> The **compiler** owns the proof of its translation class. Each **instance**
> supplies only witness data — manufactured or reference solution, boundary
> data, norm, expected order, conserved quantity, tolerances, and nonclaims.

A compiler proves consistency of its translation class. It cannot prove that a
submitted PDE is well posed, physically appropriate, or adequately
discretized — and generated residual, JVP, and VJP implementations sharing one
faulty lowering are correlated, not independent, evidence. Hand-derived oracles
therefore remain mandatory.

### First slice

Bounded claim: the exact `org.example.poisson` strong relation lowers through a
private, proof-carrying 2D Cartesian Q1 Galerkin form, produces local
matrix/RHS contributions and paired residual/JVP/VJP actions on the CPU
reference path, and agrees with independent analytic, assembled, convergence,
and conservation oracles.

Nonclaims: source-level weak forms; arbitrary expressions; natural or mixed
boundary conditions; vector, mixed, or nonlinear forms; simplex, high-order, or
adaptive spaces; native or JIT code generation; a public form IR; CUDA or MPI;
and any performance property.

The output target is `LocalContribution`, which owns a local matrix **and**
RHS. `LocalLinearActionIr` is not a sufficient target: Poisson needs a source
term, which RFC 0020 deliberately excludes from local action.

Stop condition: if Poisson requires a special case at every term, stop and
report, rather than promoting the result toward a general weak-form IR.
Proceed to a second consumer only if the private plan deletes or simplifies a
hand-written residual or derivative path **without weakening any oracle**.

## Governance amendments

Each amendment is applied in its owning document. Recorded here with its
reason so the change is auditable rather than silent.

### A1 — Abstraction budget admits audit compression

*Owner:* [vertical-slice development](vertical-slice-development.md)

The two-consumer rule is a human-team heuristic: it avoids speculative
generality because refactoring is expensive. Under agent authorship,
refactoring is cheap and **audit** is the dominant cost.

A **private** abstraction may be introduced with one consumer if it
demonstrates audit compression, meaning all of: an independent agent owns a
class-level mutant and falsifier suite; the count of invariant-bearing
hand-written formula sites does not increase; at least two hand-written
implementations, or one primal/JVP/VJP triple, are deleted; a new instance
requires only witness data rather than executable formulas; and no public type,
wire, or registry is added.

**Public or durable API keeps the stricter rule** — two external consumers, or
the public surface is itself the product claim — because there the dominant
cost is compatibility audit, which agent authorship does not reduce.

### A2 — Coordination state is external and non-authoritative

*Owner:* [`AGENTS.md`](../../AGENTS.md)

Self-driving agents need machine-readable coordination state. The current
prohibition is retained for the repository and relaxed outside it: **do not
create a durable activity ledger inside the repository; non-authoritative
external coordination state is permitted.**

External state may hold only agent, slice, base revision, branch or worktree,
current lock, and handoff. It must be disposable, must never become the
authority for a Model, claim, or evidence, must not shadow `verify/`, the Issue
queue, or the roadmap, and is never committed.

### A3 — Conformance kits split by class and instance

*Owner:* [vertical-slice development](vertical-slice-development.md)

Kits divide into **compiler-class conformance** — derivation rules, reference
interpreter, mutant corpus, primal/JVP/VJP consistency, owned by the compiler —
and **instance witness**, supplied per physics. A physics module may not add its
own kernel, Jacobian, or conformance harness when the compiler class can
express it; it returns the unexpressible requirement to the contract owner
instead. Adapter and provider conformance kits are not forced into the instance
model.

### A4 — Independence of the falsifier is mandatory, and the integrator rotates

*Owner:* [`AGENTS.md`](../../AGENTS.md)

The integrator is a per-slice role, not a permanent one. The current
"an independent agent *should* derive the falsifier" is strengthened to a
requirement: **an implementing agent must not author, tune, or relax the
oracle, expected values, tolerances, or falsifiers for its own implementation.**
Wiring a pre-committed fixture is permitted; owning the evidence content is not.

### A5 — Architecture predicates enter CI

*Owner:* new `cargo xtask check-architecture` and `tools/ci/architecture-debt.toml`

See the table below. Existing violations are recorded as ratcheted debt with a
reason and a deletion condition; ordinary pull requests may only move the
numbers down.

### A6 — The physics chain depends on the compiler lane

*Owner:* [roadmap](../roadmap.md)

The FEM form-compiler lane is inserted as a prerequisite of *broader* FEM
structural and fluid libraries. The elasticity patch, thermal slab, and
Couette–Poiseuille slices are **not** blocked: they are the lane's candidate
second consumers and falsifiers. FVM libraries are explicitly excluded from
this dependency.

### A7 — Scale gates open on a declared envelope breach

*Owner:* [library and accelerator strategy](library-and-accelerator-strategy.md)

The deferred-gate falsifier and provenance requirements are retained. An
additional, disjunctive condition for *starting investigation* is added: the
current method demonstrably breaks a **pre-declared resource, convergence, or
robustness envelope** on an existing or synthetic operator. "The absence of the
capability prevents consumers" is rejected as too subjective.

The falsifier and construction/provenance policy are built **first**; a
candidate enters the stable vocabulary only after passing. AMG, restarted
GMRES, and field split are three distinct contracts — a multilevel construction
and provenance problem, a Krylov algorithm, and a solver graph over blocks
respectively — and each needs its own envelope, not one shared gate.

## Architecture predicates

Counted from the AST, not by regular expression. **Implemented** predicates run
in `cargo xtask check-architecture` today; **planned** ones are named here so
the destination is fixed, and must not be cited as though CI enforced them.

| Predicate | Status |
| --- | --- |
| File size | implemented |
| Public surface | implemented |
| Glob re-exports | implemented |
| Dependency graph acyclicity | implemented |
| RFC numbering and index agreement | implemented |
| Function complexity | planned |
| `unreachable_pub`, duplicate canonical paths | planned |
| Cross-physics clone | planned — needs a similarity algorithm and a normalization decision |
| Role skeleton | planned — depends on the clone scanner |

| Predicate | Threshold |
| --- | --- |
| File size | New production `.rs` ≤ 1,000 physical lines; target ceiling 1,500 for production, 2,000 for tests. Existing excess frozen at its current value. Generated files exempt only with a generator path and hash. |
| Function complexity | New or changed functions ≤ 120 logical lines, cyclomatic complexity ≤ 20, nesting ≤ 5, parameters ≤ 7. Excess requires a debt entry with reason and deletion condition. |
| Public surface | New crates ≤ 128 AST-reachable public items. Every existing crate is frozen at its exact current count, so a crate under the budget cannot drift up to it unobserved. A public capability slice may add ≤ 8 net. Above 128, a freeze must carry a reason and a removal condition. |
| Visibility | Zero `unreachable_pub` violations, zero glob re-exports, zero duplicate canonical public paths. Extend the existing facade check from `eqiora` to every publishable crate. |
| Dependency graph | Every workspace SCC has size 1 across normal, build, and dev edges. Currently zero cycles; retain the existing layer-direction check. |
| Cross-physics clone | Zero new clone classes spanning distinct physics owners at ≥ 30 logical lines or ≥ 100 normalized tokens with ≥ 85% AST similarity. Existing clones recorded as digest-keyed debt. |
| Role skeleton | A role skeleton may not be newly created across three or more physics owners; consolidate into a shared typed contract or reduce it to instance witness. |
| Debt ratchet | Every entry carries path, metric, current ceiling, technical reason, and deletion condition. Raising a number or adding an entry is an architecture change under review. |

This is a set of CI-checked technical exceptions, not an activity ledger. It
integrates alongside the existing fmt, Clippy, layer, facade, and rustdoc
checks.

## Division of labor

Two agents, divided by **invariant ownership and oracle independence** rather
than by task type. One writer per central seam; an independent verifier; one
integrator per slice.

### What each agent is actually good at

Assignment follows measured and reported strengths rather than a guess, and the
first slice confirmed both profiles in practice.

| | Claude (Opus 5) | Codex (GPT-5.6 Sol) |
| --- | --- | --- |
| Agentic coding | SWE-bench Pro 79.2%, Frontier-Bench 43.3% | 64.6%, 34.4% |
| Novel reasoning | ARC-AGI-3 30.2% | 7.8% |
| Long implementation runs | — | stays oriented, follows more requirements, finishes unglamorous work |
| Work with a settled shape | — | strongest here; rewards precise prompts, punishes vague ones |
| Ambiguous judgment, multiple defensible paths | strongest here | reported weakness |
| Acting beyond what was asked | — | reported tendency; constrain with explicit write-path allowlists |

The first slice matched this exactly: a contract with unstated tolerances and
rule IDs was correctly refused, and the same contract stated precisely was
implemented without further questions.

Two consequences:

1. **Contract design, oracle derivation, and any decision with several
   defensible answers belong to Claude.** Deriving a falsifier from first
   principles is novel reasoning, which is the widest measured gap.
2. **Implementation is not Codex-only.** Claude is the stronger agentic coder,
   so both implement. Oracle independence is preserved by *cross-assignment*
   rather than by role: **whoever implements a lane, the other agent writes its
   falsifier and reviews it.** Lanes run in parallel when their writable paths
   are disjoint.

Cross-assignment is symmetric and has no exemption for the integrator. The
first slice violated this in one direction: Codex's implementation was reviewed
by Claude, but the governance amendments, the RFC, the architecture predicates,
and the scaling envelope were all written by Claude and accepted by Claude. The
failure mode is structural rather than careless — the agent holding the
acceptance decision is the one for whom self-acceptance costs nothing, so the
rule has to bind hardest exactly there. A brief review is enough; skipping it
is not.

Codex prompts must carry an explicit allowlist of writable paths and an
explicit list of integration-owned paths, because acting beyond the request is
a reported failure mode rather than a hypothetical one.

### What the billing models imply

The two agents are metered differently, and that difference — not preference —
decides the granularity of a request.

| | Claude (Max 20x) | Codex (Pro) |
| --- | --- | --- |
| Metering | five-hour rolling window plus weekly caps | credits, roughly one task at 5–45 |
| Cheap shape | many small exchanges | few large, long-running ones |
| Expensive shape | long single turns that idle | chatty round trips |

So Claude carries the high-frequency work — exploration, verification,
integration, oracle derivation, review, coordination — and may fan out many
parallel subagents. Codex carries a small number of large units: central
implementation, sustained autonomous runs, adversarial review.

**An implementation request to Codex goes out only against a fully frozen
contract.** Iterating a vague contract by round trip is the expensive shape,
and it has already cost one full cycle: a contract missing tolerances and rule
IDs was correctly refused rather than guessed at. Adversarial review of a
*contract* is exempt — that is itself a large-grained task and does not need
the contract settled first.

### Agreed execution order

Ordered by what becomes more expensive the longer it waits, not by visible
progress.

1. **Does the form compiler pay for itself** — the audit-compression verdict.
   Everything downstream is built either on a compiler that subsumes
   hand-written evaluators or on the admission that it does not.
2. **A public error norm over accepted results, and a reproducible Model
   digest** — independent slices, run in parallel. Both compound: every demo
   written before the norm exists hand-rolls one, and a non-deterministic
   digest on the shortest public path is a **contract violation** for a product
   claiming content-addressed identity, not a presentation defect.
3. **Python Model Package compilation** — the shipped packages are currently
   reachable from Python only by compiling their source text.
4. **AMG construction and provenance**, whose gate the envelope breach opened,
   and 3D reach.
5. **Demos of capabilities that already carry registered evidence.** No new
   hand-written physics; a demo never justifies a new formula site.

| Phase | Claude — integrator and oracle | Codex — central implementation |
| --- | --- | --- |
| Contract | Freeze the claim, nonclaims, roles, derivation rules, stop condition, and API budget. Refresh the Issue queue; identify registration deltas. | Adversarially review whether the contract travels through existing Kernel, Realization, basis, quadrature, and assembly types. Do not create a competing central type. |
| Oracle | Derive analytic values, signs, boundary terms, mutants, and thresholds **before** reading implementation details. | Do not author or tune expected values. Challenge them through independently run results. |
| Implementation | Do not edit the form module concurrently. Hold integration context; resolve contract questions. | Sole writer. Run sustained build, test, Clippy, and rustdoc loops, plus `local_verify.py fast` and explicit `--case` runs. |
| Evidence lane | After acceptance, own fixture prose, the public orientation path, capability-matrix wording, and cross-lane registration. | Supply proposed export, manifest, case, and matrix deltas. Do not edit integration-owned registries. |
| Acceptance | Rebase to the accepted revision, run `local_verify.py affected`, review the full diff against the pre-committed oracle, retain merge authority. | Review signs, index ordering, transpose behavior, parameter roles, and resource bounds. Return anomalies; never self-accept. |

If the contract proves insufficient, Codex stops and returns the missing
requirement; Claude updates and re-accepts the contract before implementation
resumes.

## What this document does not claim

- No schedule. The measured cost structure is a warning about the current
  growth curve, not an estimator; team and scope data to support any calendar
  figure do not exist.
- No claim that a form compiler is sufficient. Stable mixed spaces, stronger
  solvers, scale, performance baselines, physics validation, and task-oriented
  documentation remain independently open.
- No claim that the compiler will replace gauge selection, stable mixed-space
  choice, interface topology, remeshing, nonlinear acceptance, reconstruction,
  or physical conservation evidence. It will not.
- No device claim. The CUDA adapter consumes a finalized CSR and cannot consume
  generated element kernels; device residency, gather/action/scatter, and
  launch are a separate lane with separate evidence.
