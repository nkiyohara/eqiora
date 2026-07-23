# Expected evidence

The registered test accepts only when all of the following close together:

- two genuine consecutive CPU reference solves produce distinct accepted
  states at the exact fixed-step coordinates;
- all four physical FSI Fields replay from exact Model, Realization, geometry,
  correspondence, mesh, support, unit, shape, frame, and coefficient blocks;
- the fluid MINI velocity retains both vertex and cell-bubble blocks;
- state, segment, trajectory, Run output, and Dataset references preserve
  exact identity and immutable prefix publication;
- declaration-order changes reproduce identical canonical artifacts;
- a selected pressure Field can be traversed from the final trajectory without
  loading the first segment or unrelated Field values; and
- two raw chunk partitions restore one logical discrete Field while producing
  distinct storage-envelope identities.

Mutation checks reject incomplete state inventories, stale mesh lineage,
nonmonotone or duplicate accepted coordinates, noncanonical wire order,
missing trajectory segments, incomplete Dataset sources, missing chunks,
substituted chunks, and content-valid artifacts stored under the wrong digest.
