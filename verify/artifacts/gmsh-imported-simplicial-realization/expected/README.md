# Acceptance

Both official-generator fixtures import through bounded MSH 4.1 syntax,
reconstruct to equal meshes, and round-trip as identical canonical
content-addressed mesh data. `mesh.sha256` fixes the domain-separated artifact
digest. The Realization references that exact digest and
the single interior P1 degree of freedom equals `1 / 12`; total source equals
`1` and boundary reaction equals `-1` to roundoff.

Input outside the admitted syntax, semantics, resource, topology, geometry,
orientation, or quality boundary must fail before accepted execution evidence
is returned.
