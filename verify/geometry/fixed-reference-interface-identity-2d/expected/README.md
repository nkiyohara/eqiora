# Expected evidence

- Exact Model, geometry-revision, correspondence, and mesh-revision digests
  form one closed artifact chain; a stale or substituted resource fails before
  evidence.
- Fluid and solid cell subsets are nonempty, disjoint, and cover the bound
  mesh exactly once.
- For each body, its mapped exterior and interface facets are exactly the
  relative boundary of its cell subset, without duplicates or omissions.
- The distinct fluid and solid interface Boundary Domains keep distinct
  semantic parents while mapping to the same complete mesh-facet set.
- Every shared facet has one adjacent fluid cell and one adjacent solid cell;
  oriented incidence derives opposite parent-outward orientations without a
  caller-authored sign.
- Selected-body input and association-candidate order do not change canonical
  bytes for the same exact referenced resources.
- Explicit total one-to-one successor lineage retains selected intent across
  geometry revisions without assuming equal Domain ULIDs. Missing, split,
  merged, ambiguous, or digest-mismatched lineage is rejected.
- Wrong Domain identity or parent, invalid entity membership, partial
  interface coverage, and forged adjacency fail before transfer, assembly, or
  solve.
