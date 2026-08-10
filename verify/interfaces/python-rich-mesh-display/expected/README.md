# Expected observations

No screenshot, pixel value, tolerance, derived quality value, or newly encoded
scientific artifact is expected here. The normative observations are protocol,
identity, transport, interaction, lifecycle, accessibility, and distribution
facts recorded in `case.toml` and executable tests.

The exact text outcomes are:

- ordinary/optional-absence: the unchanged `repr(mesh)`;
- unsupported reference:
  `repr(mesh) + "\nNotebook view unavailable: this N1 viewer supports only the exact accepted 50-chord circular-hole reference Mesh (104 vertices, 104 triangles)."`;
- corrupt runtime/assets:
  `repr(mesh) + "\nNotebook view unavailable: the installed Eqiora Notebook presentation runtime or assets are incomplete. Reinstall eqiora[notebook]."`.

A diagnostic exists only when `text/plain` survives filtering. Widget-only
failure returns `{}`. Every excluded, empty, absent, unsupported, or corrupt
outcome creates zero surviving comms.

The admitted private payload has exactly six immutable Eqiora members:
`profile`, `mesh_digest`, `vertex_count`, `triangle_count`,
`coordinates_f64_le`, and `triangles_u32_le`. Their literal identity, counts,
byte lengths, accepted bytes, and explicit little-endian decoding are fixed by
the existing Mesh owner. Camera and mode state are per-view and never enter
this payload.

The independent mutants include filter-order reversal, invalid-argument side
effects, tuple return, comm-on-failure, same-shape source admission, raw/public
digest substitution, endian and same-size byte drift, count/index/degeneracy
drift, client writes/messages, remote resources, swapped/no-op camera actions,
missing named views/modes, pointer-only or color-only controls, permanent
frames, partial cleanup, fresh comm per view, stale delegate reuse, blank WebGL
failure, and pixel-only acceptance. None may be relaxed after writer output is
observed.

H2's exact asset, notice, license, lock, browser, and candidate-bound receipt
values do not exist in this directory. They are produced and checked only by
the separate post-writer release-trust lane; an absent or failed H2 remains a
hard integration failure.
