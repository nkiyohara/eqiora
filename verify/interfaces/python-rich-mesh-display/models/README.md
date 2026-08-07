# Model inputs

This case owns no Model, equation, solver, or new Mesh fixture. Its sole data
input is the installed native reference Mesh produced through the exact public
path already verified by
`interfaces.python-circular-hole-chordal-mesh`.

Both Notebook fixtures construct that accepted Mesh through
`CadAuthoredGraph -> Geometry -> MeshRequest -> resolve -> generate`. They do
not vendor canonical Mesh bytes, reconstruct topology, call a renderer wrapper,
or substitute a raw-array object. The distinct same-shape source mutant is
constructed through the same producer with swapped inlet/outlet source roles;
it retains the accepted inner Mesh bytes and shape while changing exact source
identity, and therefore must remain text-only.
