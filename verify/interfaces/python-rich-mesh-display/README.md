# Python rich Mesh display

This case verifies one deliberately closed Notebook presentation path. An
installed native `eqiora.meshing.Mesh` carries the conventional
`_repr_mimebundle_(include=None, exclude=None)` hook, but only the already
accepted 50-chord circular-hole reference Mesh may create rich output. A bare
last expression renders through the same private anywidget adapter in exact
JupyterLab 4.6.2 and marimo 0.23.16.

The case consumes, rather than re-derives, the exact source, canonical-byte,
and Mesh identities owned by
[`interfaces.python-circular-hole-chordal-mesh`](../python-circular-hole-chordal-mesh/README.md).
It adds no scientific number, tolerance, topology acceptance, or pixel oracle.

## Ordinary path

```text
accepted native Mesh
  -> native _repr_mimebundle_ filtering and exact-reference admission
  -> private Python owned-copy/immutable anywidget payload
  -> wheel-local Three.js frontend
  -> bare JupyterLab or marimo output
```

Text-only, empty, excluded-widget, absent-extra, unsupported-profile, and
corrupt-runtime outcomes happen before a comm can survive. The accepted rich
outcome normalizes anywidget's tuple to one public data dictionary containing
the exact Mesh `repr` and conventional widget-view MIME only.

Each live Mesh owns at most one open delegate. Multiple output views share its
model and comm but own independent camera, mode, DOM, listeners, frames, and
GPU resources. Removing a view cleans only that view. Closing the comm cleans
all remaining views exactly once. A closed delegate is terminal; redisplay
creates a fresh model rather than calling it again. The host may retain the
delegate and output after the outer Mesh has been collected because the copied
payload holds no Mesh reference.

## Independent evidence

The Python oracle fixes protocol filtering, exact fallbacks, zero-comm failure,
same-shape source drift, open reuse, close/fresh behavior, immutable synced
members, and byte-for-byte Mesh preservation. The frontend oracle replays the
accepted little-endian coordinate/connectivity bytes and rejects endian,
length, non-finite, digest, count, index, degeneracy, and same-size drift before
renderer creation.

The two real-host fixtures leave `mesh` itself as the display expression. The
browser oracle separately observes orbit, pan, zoom, reset, top, isometric,
surface, wireframe, points, keyboard focus, text alternative, local-state
independence, output cleanup, comm close, fresh redisplay, WebGL failure, and
loopback-only traffic. A private root-bound observation seam reports camera and
resource state only to this test; it is not synced, persisted, exported by
Python, or a public viewer protocol.

Distribution activation, candidate v3/H2 identity, the exact lock graph,
deterministic asset rebuild, wheel inventory, licenses/notices, and managed
browser bytes remain owned by the separate release-trust oracle. This case uses
the same `python-installed-wheel` candidate runner and the exact `notebook`
profile; it creates no second build identity.

That shared candidate runner accepts host teardown only after bounded cleanup
returns a complete-empty owned-process observation without forced escalation.
The preserved host-status predicate accepts status 0, or exactly `-SIGTERM`
only when the candidate runner requested SIGTERM; an unsolicited signal, any
other nonzero status, timeout, or forced kill still rejects.
The graceful phase is bounded by 30.0 seconds and all later escalation,
reaping, and observation share the same absolute 35.0-second cleanup decision
deadline. On a primary host failure cleanup still runs; a survivor, incomplete
observation, authority denial, overflow, escalation, or deadline is a bounded
rejection with stable diagnostics. The case does not claim that an
uninterruptible or inaccessible survivor disappears within a fixed time.

## Boundary

This is not arbitrary Mesh display, a public `view()` or widget type, a
renderer registry, a Studio component, selection/picking, field or trajectory
visualization, scientific evidence from pixels, saved media, or a browser and
platform compatibility promise. Camera and representation state are private
presentation state and never mutate Python.

The registered gate is:

```bash
python3 tools/ci/python_distribution_gate.py
cargo run --locked -p eqiora-verify -- run \
  --case interfaces.python-rich-mesh-display
```

The standalone oracle branch is intentionally red at the frozen predecessor:
that revision has no hook, frontend source/package inputs, optional profile, or
candidate v3 implementation. Red caused by those missing writer/integrator
inputs is expected; changing oracle observations to make the predecessor pass
is forbidden.
