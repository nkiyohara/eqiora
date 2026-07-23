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

The accepted child resolves the ordinary host-serial finite-element
Realization and executes the existing scalar-elliptic application path. The
registered oracle is a verification-only record, not a public agent or
evidence schema. It independently checks the complete primary Field for the
four-cell Poisson problem. A second proposal compiles, commits, resolves,
solves, and satisfies algebraic and balance acceptance, but fails that
scientific oracle. Solver success and agent claims therefore cannot admit it.

Exact Model, Realization, Run v2, complete Field values, and the sealed
execution-output fingerprint replay. Falsifiers reject a stale plan, a plan
from a different same-revision sibling Model even when its Transaction digest
matches, a foreign Model/Realization binding, forged execution provenance,
forged Run outputs, an unsupported worker request before numerical allocation,
a no-op, and a non-finite edit. Every rejected mutation leaves the selected
base unchanged.

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
