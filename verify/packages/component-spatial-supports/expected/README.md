# Expected evidence

The integration test owns the exact typed assertions; there is no derived
snapshot to refresh.

For the local fixture it requires two distinct occurrence identities, exact
`BoundaryOf`, `DefinedOn`, and `AppliesOn` edges, no support-slot Kernel entity
or display alias, and complete support-binding provenance. For the exact
package path it requires two-level slot forwarding and identical canonical
Model bytes and digests under two dependency-alias spellings.

The negative matrix does not freeze a complete diagnostic snapshot. Each
invalid definition or binding must reach the intended typed diagnostic before
any Model, Transaction, or graph mutation can be observed.
