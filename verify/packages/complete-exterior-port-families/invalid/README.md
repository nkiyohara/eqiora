# Invalid complete-exterior cases

Each source is a standalone falsifier for one RFC 0041 contract. The Rust
integration test owns the diagnostic assertions and requires compilation to
return no model. These fixtures intentionally remain outside `models/` so a
verification runner cannot mistake expected rejection for positive evidence.

- `missing-side.eqi`: an explicit set is not a complete Cartesian exterior.
- `duplicate-exact.eqi`: one exact Boundary identity appears twice.
- `duplicate-geometry.eqi`: distinct identities claim the same Cartesian side.
- `wrong-parent.eqi`: one member has a different exact parent volume.
- `volume-member.eqi`: an exact volume is supplied where a Boundary is required.
- `boundary-of-boundary.eqi`: a boundary attempts to use another boundary as
  its parent.
- `wrong-dimension.eqi`: a 2D occurrence attempts to satisfy a 3D obligation.
- `empty-set.eqi`: an explicit set contains no exact Boundary identity.
- `non-boundary-selector.eqi`: a volume is used as a family selector.
- `selector-outside-bound-set.eqi`: a geometrically coincident but unbound
  Boundary identity is used as a selector.
- `distinct-connector.eqi`: two structurally equal but nominally distinct
  field-physical Connectors are connected.
- `unconnected-family.eqi`: a valid complete exterior has no Connection;
  family membership therefore remains open rather than being invented.

Independent membership and checked expansion limits are exercised by
generated source in the integration test; committing tens of thousands of
repeated tokens would add repository weight without strengthening the claim.
