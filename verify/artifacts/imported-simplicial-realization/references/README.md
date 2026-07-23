# Reference provenance

The mesh authority is the typed `SimplicialMesh` reconstructed from canonical
artifact bytes. Its connectivity, positive orientation, mean-ratio gate, and
quality evidence are recomputed by the same L2 meshing contract used by
numerical assembly. The scalar solution reference follows directly from the
one-row assembled P1 system on four congruent triangles.
