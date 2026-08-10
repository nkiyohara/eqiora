# References and sealed authority

- RFC 0071 owns the three-generator spatial-periodic group and packet
  conventions.
- `../cartesian-periodic-topology-3d/models/periodic-box.eqi` and its permuted
  sibling are reused directly; this case adds no `.eqi` source.
- Accepted evidence contract v3 SHA-256:
  `43c389e6b12b57c3363dc1670bddabd2d250d0d7ef8cfdc5846961086af29b3d`.
- Focused ACCEPT of that contract SHA-256:
  `7c7208e5f62f2d8ffa5f39459518c383fd73c21745110f6bd35ff37e64f297d1`.
- Exact-current-head returned-interface decision v2 SHA-256:
  `09770644563a8436f077bd21b0d4efeb7e4a2ec344a9a3bafb3fad5d0edacb67`.
- Focused ACCEPT of that decision SHA-256:
  `a0b0f8eb6d7ac687bda6a1404c7ba60b8233736fa773b005d1b7dbdfc928e61a`.

The test may observe only the frozen private view, projection result, event
stream, and inventory receipt. It must not call or copy production `derive_*`
or `admit_*` helpers, reconstruct those observed receipts, introduce a second
production validator, or infer any of this case's expected values from product
output.
