# AI-authored platform strategy

- Status: Accepted
- Created: 2026-07-25
- Baseline revision: `f5ae8c5`
- Related: [library and accelerator strategy](library-and-accelerator-strategy.md),
  [contract-wave capability development](vertical-slice-development.md),
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

Every arrow in the roadmap's physics chain has been a separately hand-written
capability path. Under the premise above, that chain encodes the wrong growth
curve: each new physics re-earns its proof machinery instead of inheriting it.
The lane's purpose is therefore **amortizing evidence**, not adding physics
faster.

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

*Owner:* [contract-wave capability development](vertical-slice-development.md)

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

External state may hold only agent, lane, base revision, branch or worktree,
current lock, and handoff. It must be disposable, must never become the
authority for a Model, claim, or evidence, must not shadow `verify/`, the Issue
queue, or the roadmap, and is never committed.

### A3 — Conformance kits split by class and instance

*Owner:* [contract-wave capability development](vertical-slice-development.md)

Kits divide into **compiler-class conformance** — derivation rules, reference
interpreter, mutant corpus, primal/JVP/VJP consistency, owned by the compiler —
and **instance witness**, supplied per physics. A physics module may not add its
own kernel, Jacobian, or conformance harness when the compiler class can
express it; it returns the unexpressible requirement to the contract owner
instead. Adapter and provider conformance kits are not forced into the instance
model.

### A4 — Oracle independence is mandatory; review follows durable risk

*Owner:* [`AGENTS.md`](../../AGENTS.md)

The integrator is a per-integration-envelope role, not a permanent one. The
current
"an independent agent *should* derive the falsifier" is strengthened to a
requirement: **an implementing agent must not author, tune, or relax the
oracle, expected values, tolerances, or falsifiers for its own implementation.**
Wiring a pre-committed fixture is permitted; owning the evidence content is not.

Fresh-context non-writer review of the complete risky delta is mandatory for
changes to governance, review, or evidence policy; scientific meaning or
evidence; public or versioned API and compatibility; persisted schemas or exact
artifacts; security or data integrity; release or CI trust; and architecture
ceilings or debt. Outside those boundaries, non-governance documentation,
dependency-only updates with their lockfile and relevant gate, reproducible
generated or mechanical changes, private behavior-preserving refactors, and
localized corrections to low-risk findings integrate by integrator self-review
and repository gates absent an anomaly. A mixed change sends only its risky
delta to review. A strictly localized correction to a reviewed high-risk
finding gets focused correction-only fresh review; if it changes claim,
evidence, or compatibility, widens scope, or otherwise reopens accepted risk,
review the reopened risky delta plus needed context as a new high-risk change.

### A5 — Architecture predicates enter CI

*Owner:* new `cargo xtask check-architecture` and `tools/ci/architecture-debt.toml`

See the table below. Existing violations are recorded as ratcheted debt with a
reason and a deletion condition; ordinary pull requests may only move the
numbers down.

### A6 — The physics chain depends on the compiler lane

*Owner:* [roadmap](../roadmap.md)

The FEM form-compiler lane is inserted as a prerequisite of *broader* FEM
structural and fluid libraries. The elasticity patch, thermal slab, and
Couette–Poiseuille capability closures are **not** blocked: they are the
lane's candidate second consumers and falsifiers. FVM libraries are explicitly
excluded from this dependency.

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
| Public surface | New crates ≤ 128 AST-reachable public items. Every existing crate is frozen at its exact current count, so a crate under the budget cannot drift up to it unobserved. A public capability closure may add ≤ 8 net. Above 128, a freeze must carry a reason and a removal condition. |
| Visibility | Zero `unreachable_pub` violations, zero glob re-exports, zero duplicate canonical public paths. Extend the existing facade check from `eqiora` to every publishable crate. |
| Dependency graph | Every workspace SCC has size 1 across normal, build, and dev edges. Currently zero cycles; retain the existing layer-direction check. |
| Cross-physics clone | Zero new clone classes spanning distinct physics owners at ≥ 30 logical lines or ≥ 100 normalized tokens with ≥ 85% AST similarity. Existing clones recorded as digest-keyed debt. |
| Role skeleton | A role skeleton may not be newly created across three or more physics owners; consolidate into a shared typed contract or reduce it to instance witness. |
| Debt ratchet | Every entry carries path, metric, current ceiling, technical reason, and deletion condition. Raising a number or adding an entry is an architecture change under review. |

This is a set of CI-checked technical exceptions, not an activity ledger. It
integrates alongside the existing fmt, Clippy, layer, facade, and rustdoc
checks.

## Division of labor

Three agents, divided by **invariant ownership, observed failure mode, and
oracle independence** rather than a static task taxonomy. One writer owns each
central seam; a fresh-context non-writer supplies the independent oracle and,
for durable-risk deltas, review; one integrator owns each integration envelope.

### Routing from evidence, not reputation

Anthropic describes [Fable 5](https://www.anthropic.com/claude/fable) as its
highest-capability generally available model for long-running agents and
[Opus 5](https://platform.claude.com/docs/en/about-claude/models/choosing-a-model)
as the lower-cost choice for complex agentic coding. Those are starting priors,
not repository evidence. Eqiora routes work from failures observed on its own
contracts:

| Agent | Strong observed use | Failure mode to guard | Default brief |
| --- | --- | --- | --- |
| Claude Opus 5 | bounded analytic/numerical derivation; refusing an ambiguous oracle rather than guessing; plausible-mutant search | successive review passes may reveal a new mutant each time; a broad implementation may produce substantial surface code before proving its live producer and lineage | complete frozen specification, writable-path allowlist, hard scope/length limit, precommitted probes, exact terminal verdict |
| Claude Fable 5 | provisional escalation for the hardest long-horizon, cross-cutting, UI/vision, and review-convergence work; official capability lead, with Eqiora outcomes recorded as they land | higher cost and no exemption from independent evidence; safety routing or refusal must fail closed | whole goal plus invariant owner, live consumer, nonclaims, stop conditions, visual/runtime oracle, and repository gate |
| Codex GPT-5.6 Sol | contract ownership; navigation through existing architecture; live end-to-end lineage; integration and complete repository gates | may widen authority or continue beyond an underspecified boundary unless ownership and write paths are explicit | goal, context, authority, constraints, done-when, integration-owned paths, and exact verification path |

The routing rule is operational:

1. Give Opus a bounded route when inputs, outputs, and stop conditions are
   frozen. Its independent derivation and mutation search remain valuable even
   when a first implementation attempt would be too open-ended.
2. Escalate to Fable when the task crosses several ownership seams, needs
   rendered/visual judgment, must sustain a long implementation, or a bounded
   Opus review returns successive newly discovered mutants instead of
   converging. Do not use Fable merely to repeat an unchanged prompt.
3. Keep Codex on contract ownership and integration, and prefer it for a
   settled lane whose main risk is losing the live Model--Realization--Run--
   consumer lineage while navigating existing code.
4. Re-evaluate these priors after concrete Eqiora outcomes. One successful or
   failed run changes a probe, not a permanent model label.

Implementation is not assigned exclusively to any model. Oracle independence
is preserved by role separation: whoever implements, a fresh-context
non-implementer derives the falsifier; two independent routes derive any new
scientific formulation, expected value, or tolerance. An implementation writer
owns neither route of its dual oracle. Provider diversity is an escalation on
disagreement or consequential science, not a gate. Disjoint read-only lanes
may run in parallel; disjoint writable lanes additionally require frozen
contracts.

Required review is likewise role-independent and has no exemption for the
integrator's own durable-risk work. The first governance revision exposed the
structural failure mode: the agent holding acceptance can accept its own consequential
governance, architecture, and trust decisions at no marginal cost. The remedy
is independent review of those complete risky deltas, not blanket rereview of
mechanically gated low-risk changes. A localized high-risk correction gets
focused fresh review; only reopened risk returns as a new high-risk delta.

Codex prompts carry explicit writable and integration-owned path lists. Claude
prompts carry the same lists when they write; model capability never broadens
authority.

### What model cost implies

Anthropic currently prices Fable 5 at twice Opus 5 per token. Cost decides
which already-qualified lane gets the stronger escalation, never whether a
claim receives independent evidence. Opus is the ordinary bounded Claude lane;
Fable is the capability-first escalation above. Codex remains suited to fewer,
larger integration units under its different metering. Recheck current
availability and pricing rather than preserving them as architecture.

An implementation request to any model goes out only against a fully frozen
contract. Adversarial review of the contract is exempt: discovering what must
be frozen is the bounded output of that lane.

### Agreed execution order

Ordered by what becomes more expensive the longer it waits, not by visible
progress.

1. **Does the form compiler pay for itself** — the audit-compression verdict.
   Everything downstream is built either on a compiler that subsumes
   hand-written evaluators or on the admission that it does not.
2. **A public error norm over accepted results, and a reproducible Model
   digest** — independent lanes, run in parallel. Both compound: every demo
   written before the norm exists hand-rolls one, and a non-deterministic
   digest on the shortest public path is a **contract violation** for a product
   claiming content-addressed identity, not a presentation defect.
3. **Python Model Package compilation** — one exact installed-package path now
   consumes an explicit content-addressed store, canonical resolution bytes,
   and a root-local Model selector through the Rust-owned package compiler.
   Discovery, package authoring/installation, registries, and Studio remain
   later independent surfaces.
4. **AMG construction and provenance**, whose gate the envelope breach opened,
   and 3D reach.
5. **Demos of capabilities that already carry registered evidence.** No new
   hand-written physics; a demo never justifies a new formula site.

| Phase | Owner | Independent route |
| --- | --- | --- |
| Contract | The contract-cell owner, normally Codex, freezes claim, nonclaims, live consumer, derivation rules, stop condition, API budget, and registration deltas. | A non-writing route challenges bounded scientific ambiguity and cross-seam architecture before the writer is selected. |
| Oracle | Fresh-context agents independent of the intended writer derive values, signs, mutants, and thresholds before reading implementation. New scientific formulations, expected values, and tolerances use two independent analytic and numerical or symbolic routes; the writer owns neither. | The contract owner checks that the oracle binds the executable seam without authoring or tuning expected values. |
| Implementation | Codex owns the settled existing-architecture path; Fable owns an escalated long-horizon or visual path; Opus may own a narrow fully frozen path. | A fresh-context non-implementer owns the falsifier and, where durable risk requires it, reviews the complete risky delta. |
| Acceptance | The per-envelope integrator rebases, runs `local_verify.py affected`, and audits registrations and environment limitations. It may merge after the applicable risk review; a localized high-risk correction gets focused fresh review, while reopened risk is reviewed as a new high-risk delta. | For high-risk deltas, a non-writing agent checks signs, indices, lineage, visual/runtime output where applicable, and every precommitted falsifier. Low-risk deltas need no fresh route absent an anomaly. |

If the contract proves insufficient, the writer returns the missing requirement;
the contract owner re-freezes it before implementation resumes.

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
