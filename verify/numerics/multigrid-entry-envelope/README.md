# Multigrid entry envelope, declaration only

This case declares. It measures nothing, and it opens nothing.

The deferred-gate entry condition in
[the library and accelerator strategy](../../../docs/development/library-and-accelerator-strategy.md)
requires the declaration to be **merged to the protected default branch on its
own**, and admits as the breach only a hosted-CI measurement from a descendant
of that merge, started after it. Protected-branch merge order is the first thing
an author cannot rewrite; commit timestamps are self-reported and add nothing.

[`numerics.preconditioner-scaling-envelope`](../preconditioner-scaling-envelope/README.md)
does not meet that condition and records so itself:
`declared_before_measurement = false`, `declaration_provenance =
"asserted-not-auditable"`, `declaration_and_observation_share_a_commit = true`.
Its measurement stands — Jacobi removes no iteration on this operator class, and
the count grows at the unpreconditioned order. What does not stand is the
entitlement that measurement was said to confer.

## What this declaration adds, and what it does not

It fixes the probe. The strategy is explicit that choosing the threshold and
choosing the probe are both post-hoc degrees of freedom, and that freezing the
numbers closes only the first. The earlier case froze its numbers and left probe
selection open, which is the gap this closes: the source model is pinned by
SHA-256, the refinement sequence is fixed including a level nobody has measured,
the solver controls and boundary treatment are stated, and the phase
observations are named before anything is timed.

It does not restore blindness on the thresholds. Those are carried forward
unchanged from a declaration made with visibility of a measurement at `n <= 32`,
and the manifest says so in `threshold_provenance`. Re-declaring numbers that
were already chosen with data in view would be theatre if it were presented as
anything more.

`n = 48` is the one level that has never been measured at any profile, so it is
the only genuinely blind point in the sequence. It is also where the declared
predicates are decided, because the terminal ratio and slope read the last step.

## Why the phases are declared

A wall-clock total cannot say where the cost is, and a gate opened on an
unattributed total licenses the wrong work. Assembly, finalization, solve, and
peak resident bytes are therefore declared as observations before measurement.

They are observations, not thresholds. No adequacy or breach predicate reads
them. Their purpose is to decide what the *response* to a breach should be: a
better preconditioner on an assembly that dominates the runtime changes nothing,
and the phase split is the only thing that distinguishes those two worlds ahead
of committing to either.

## What opens the gate

A separate case, descending from the merge of this one, running the declared
sequence on hosted CI at the hosted build profile. If that run breaches the
declared envelope under the stated validity conditions, the multigrid gate opens
for investigation — and opening a gate is still not an implementation, nor a
claim that multigrid fixes the growth.

If the run lands in the indeterminate band, the gate stays shut and the band is
reported as declared. A void run is retained rather than discarded, and its
replacement is declared under this same two-stage rule.
