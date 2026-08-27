# Agent-authored model-change verification

This case proves that an offline implementation agent may propose a bounded
model change without gaining a second Eqiora semantics or any authority over
scientific acceptance.

The deterministic fixture proposes only a coherent-SI scalar value. The
ordinary generated compile request creates the base Model; Eqiora resolves the
target alias and derives the exact `ValueEditPlan`. Preview exposes the
versioned Transaction and binds the exact base Model digest, graph revision,
old value, and target. Atomic commit produces an immutable child through the
same public operation used by an ordinary client. The base remains unchanged,
and the two routes produce the same canonical child.

The accepted child is replayed exactly, then this evidence case invokes the
existing low-level native 1D finite-element numerical owner. That solve is a
verification-only operation, not a public Plan, Run, agent, or evidence schema.
The registered oracle independently checks the complete primary Field for the
four-cell Poisson problem. A second proposal compiles, commits, solves, and
satisfies algebraic acceptance, but fails that scientific oracle. Solver
success and agent claims therefore cannot admit it.

The exact edited Model and complete verification Field replay. Falsifiers
reject a stale edit plan, a plan from a different same-revision sibling Model
even when its Transaction digest matches, a no-op, a non-finite edit, and an
edit of the wrong semantic entity. Every rejected mutation leaves the selected
base unchanged. This case makes no public numerical lifecycle or persisted Run
lineage claim.

This slice adds no agent AST, DTO, transaction, Model schema, validator,
artifact wire, direct store access, live language-model dependency, general
model synthesis, or approval workflow. Agent implementation metadata is not
correctness evidence. A future second durable cross-process consumer may
justify an agent-neutral edit command; this case does not pre-empt that
decision.

Run:

```bash
cargo test --locked -p eqiora --test agent_authored_model_change
cargo run --locked -p eqiora-verify -- run --case interfaces.agent-authored-model-change
```
