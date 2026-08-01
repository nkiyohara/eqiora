# Inputs

The registered Rust test constructs the frozen coherent-SI graph inputs through
the public typed constructors.  This contract is below Semantic Model binding,
so it intentionally has no `.eqi` Model fixture and no provider source file.

Witness A uses `x=[-2,3] m`, `y=[-1,2] m`, `z0=0.5 m`, depth `4 m`, and
requested modeling tolerance `1e-9 m`.  Witness B changes only the tolerance to
`2e-9 m`; witness C changes only the depth to `5 m`.
