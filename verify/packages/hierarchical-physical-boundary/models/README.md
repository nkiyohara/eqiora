# Fixtures

- `components.eqi` is the dependency package. `ResistiveBranch` forwards two
  public physical Ports to one private resistor occurrence.
- `nary.eqi` closes the imported negative boundary with one three-terminal
  fragment.
- `partitioned.eqi` describes the same negative net with two overlapping
  fragments.
- `invalid-unclosed.eqi` instantiates the imported branch without closing its
  public boundary and must not produce a package release.
