# Expected relations

The [public compile schema](../../../../crates/eqiora-api/schemas/compile-v2.schema.json)
declares the current Model and Transaction schemas. The registered Rust test checks
those declarations, source/native structural equivalence, and exact edit/replay
bytes and digests. This case keeps no separate version mapping.
