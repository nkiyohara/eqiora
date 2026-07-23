# Reference provenance

The ASCII and binary fixtures were emitted from the adjacent checked-in `.geo`
source with the official Linux build:

```bash
gmsh-4.15.2 unit-square-cross.geo -2 -format msh41 \
  -o unit-square-cross.msh
gmsh-4.15.2 unit-square-cross.geo -2 -format msh41 -bin \
  -o unit-square-cross-binary.msh
```

The generated mesh contains five nodes, its point/line boundary elements, and
four full-dimensional linear triangles. Whitespace-only normalization was
applied when checking in the ASCII fixture. The checked-in source-byte digests
are deliberately distinct:

```text
18af6fe4e063a27e21c3ded75832a727147db89e9581cd978ebbd272f7b11024  unit-square-cross.msh
7e46ba6e0fb94fc7813e754f069e586ef5306445ca23e6cfa22da0790a919fd9  unit-square-cross-binary.msh
```

The mesh authority after import is the validated `SimplicialMesh`, not either
source representation, the Gmsh version, or the source path.

The scalar solution follows directly from the one-row assembled P1 system on
four congruent triangles. The Gmsh fixture is therefore an input-boundary
witness; the exact discrete oracle remains independent of Gmsh.
