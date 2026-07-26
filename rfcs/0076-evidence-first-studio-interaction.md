# RFC 0076: Evidence-first Studio interaction

- Status: Draft
- Authors: Claude (design)
- Created: 2026-07-26
- Related RFCs and evidence: RFC 0016 (Studio as an accessible canonical
  projection), RFC 0036 (physical exposure projection artifacts),
  RFC 0058 (portable Realization and bound execution graphs),
  [AI-authored platform strategy](../docs/development/ai-authored-platform-strategy.md),
  [capability matrix](../docs/capability-matrix.md)

## Summary

Every quantity Studio displays carries a reachable path to the Model,
Realization, Run, and registered evidence that produced it, and the interface
never renders a verified result and an unverified one the same way.

## Motivation

RFC 0016 already owns Studio's ownership path, canonical and workspace state,
editing and concurrency, spans, and a detailed WCAG 2.2 AA contract. This RFC
does not restate any of it. It addresses the one thing that contract does not
cover: **what a number on screen means, and how a user finds out.**

That gap matters more here than in other CAE products. Eqiora's distinguishing
property is that correctness is machine-checkable end to end — capability
tuples, fail-closed admission, independent residual acceptance, registered
evidence with explicit non-claims. A GUI that renders `2.47e-3` as a bare
number discards exactly the property the rest of the system exists to
establish. The user is then in the same position as with any other solver:
trusting the vendor.

There is a second reason, specific to this project's premise. When agents
extend the physics, the interface is where a human spot-checks their work. An
interface that cannot distinguish a verified capability from an admissible-but-
unverified one gives that reviewer nothing to check against.

The capability matrix already records, per capability, four independent gates
and an exact current boundary. Today that information lives in a document
nobody reads at the moment they are looking at a result. This RFC moves it to
the point of use.

## Proposed design

### The provenance path

Every displayed quantity — a scalar, a field sample, a plotted series, a table
cell, an exported value — resolves to a **provenance path** with four segments:

| Segment | Content |
| --- | --- |
| Model | canonical Model identity and the exact source spans that define the quantity |
| Realization | the capability tuple actually admitted: method, space, quadrature, solver, preconditioner, scalar type, reduction policy, placement |
| Run | Run identity, execution provenance, and the independently recomputed acceptance that admitted the result |
| Evidence | the registered case supporting this capability class, its claim, and its stated non-claims — or the explicit absence of one |

The path is reachable from the quantity itself, not from a separate panel the
user must know to open. It is keyboard-reachable and announced, never a
hover-only affordance: hover-only provenance would fail RFC 0016's contract and,
worse, would make the product's central property inaccessible to exactly the
users who most need an explicit rendering.

Nothing here creates a new authority. The provenance path is a projection over
artifacts that already exist, in the sense RFC 0036 fixes: a cut, not an alias.
Studio does not store, summarize, or recompute provenance; it displays what the
Run already carries.

### Three display states that never collapse

This mirrors the two-state evidence model in the platform strategy, plus the
degenerate case.

| State | Meaning | Rendering |
| --- | --- | --- |
| **Verified** | a registered case supports this exact capability class | full weight, no qualifier |
| **Admissible** | the configuration was admitted and executed, but no registered case covers this class | persistently marked, with the gap named |
| **Stale** | the Model, Realization, or inputs changed after this result was produced | marked and non-actionable until re-run |

**Only two are reachable today, and the implementation must not fake the third.**
Nothing a Run carries says whether a registered case covers its capability
class. The first implementation of this contract mapped *verified* onto
`acceptance.independentVerifier`, which is a narrower fact — whether a second
numerical backend re-checked the solve — and which the wire pins to `false`. That
mapping would have labelled every result in the product "unverified" while
appearing to have measured something, and its unit test only passed because it
cast past the schema.

So the distinction implemented today is **accepted** versus **stale**, and the
evidence segment is displayed as *unavailable, with the reason*. Per
"Compatibility and migration" below, the missing linkage is a requirement
returned to the owning contract, not something Studio reconstructs from a
convenient neighbouring field.

A number must never move from admissible to verified by being displayed in a
context that looks confident. Concretely: the marking travels with the value
into tables, plot legends, tooltips, exports, and copy-to-clipboard. A value
copied out of Studio carries its state in the copied text.

Following RFC 0016, state is never conveyed by color alone. Each state has a
text label, a non-color visual mark, and an accessible announcement.

Staleness is fail-closed, matching the rest of the system: an ambiguous or
unresolvable relationship between a result and the current Model renders as
stale, never as current. Studio does not guess that an edit was harmless.

### Non-claims are first-class

The capability matrix's "current boundary" text is not marketing copy; it is
the honest statement of what a capability does *not* cover. It appears in the
provenance path's Evidence segment verbatim, not paraphrased and not
truncated to fit a tooltip.

Where a result comes from a capability whose maturity gate is unmet — the
common case today — the interface says which gate, using the matrix's own
C/X/V/M vocabulary rather than a Studio-local invention.

### Dimensions are never dropped

The surface language is dimensioned (`1 / m ^ 2`, not `19.739`). A quantity
displayed without its unit has lost information the Model carried. Units render
with every value, in exports, and in copied text. A unitless display is
permitted only where the quantity is genuinely dimensionless, and that is
itself the declared dimension.

### Scope of this RFC

This RFC fixes the interaction contract. It does not specify a visual design
system, a component library, a chart library, a 3D viewport, or a layout beyond
what RFC 0016 already fixed. Those are separate slices and must consume this
contract rather than reinterpret it.

It also does not add a results database, a study manager, or any aggregation
across Runs. Aggregation is where provenance is most easily lost, and nothing
in the current product needs it.

## Alternatives considered

**A results dashboard that aggregates numbers across Runs.** Rejected for now.
It is the conventional CAE answer and it is where provenance dies: once a value
is averaged, plotted against another Run, or summarized into a KPI, the path to
its Realization and evidence is broken and no interface affordance restores it.
If aggregation is later required, it must carry a rule for combining provenance
and for the resulting state when the inputs disagree — a real design problem,
not a rendering one, and out of scope here.

**Provenance in a dedicated inspector panel only.** Rejected. It is cheaper and
it is what most tools do, but it makes the property opt-in: a user who does not
know the panel exists sees the same bare numbers as any other product. The
value of a machine-checkable pipeline is only realized if the check is visible
where the claim is made.

**Rendering verification status with color and an icon only.** Rejected against
RFC 0016's existing contract, and independently: status conveyed without text
does not survive copy, export, or screen readers, which is precisely when a
misread costs most.

## Compatibility and migration

No canonical semantics change, no artifact wire change, no new Kernel entity.
The provenance path reads existing Run and Realization identity; if a segment
is not currently reachable from what a Run carries, that is a finding to return
to the owning contract rather than something Studio reconstructs locally.

Existing Studio views gain state marking and the provenance affordance
incrementally. A view that cannot yet resolve a segment shows that segment as
unavailable with the reason, which is honest, rather than omitting it, which
reads as verified.

## Verification

- A displayed quantity with no reachable provenance path is a defect; the
  projection test asserts reachability for every quantity a view can render.
- An admissible result rendered without its marking is a defect, including
  after copy, export, and plot-legend paths — each is asserted separately,
  because these are exactly the paths where marking is dropped in practice.
- A stale result that renders as current is a defect. The falsifier edits the
  Model underneath a displayed result and asserts the transition.
- A quantity rendered without its unit is a defect.
- Keyboard-only and screen-reader traversal reach every provenance segment,
  asserted under RFC 0016's existing accessibility suite rather than a new one.
- The Evidence segment's non-claim text is compared against the capability
  matrix source, so a paraphrase or truncation fails.

## Security, safety, and governance

No new trust boundary, no unsafe code, no irreversible action. The provenance
path displays only what a Run already carries and grants no additional
filesystem, network, or execution authority.

One governance note: this RFC deliberately makes it harder to present an
unverified result as a verified one. That is the intent. If a future slice
needs to relax a marking, that is a capability-matrix change with an evidence
argument, not a UI preference.

## Unresolved questions

- Which segment does a quantity derived from several Runs report, once
  aggregation exists? Deferred with aggregation itself.
- Should the provenance path be exportable as a standalone artifact for a
  report, and if so does that create a compatibility promise the wire does not
  yet make?
- Does a field visualization mark state per sample, per field, or per view?
  Per sample is most honest and probably unreadable; this needs a real
  interaction study rather than a guess.
