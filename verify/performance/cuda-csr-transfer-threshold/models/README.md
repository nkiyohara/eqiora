# Measured operator

For refinement `n`, the fixture is the row-major `n` by `n` interior grid with
diagonal value four and negative unit couplings to existing axial neighbors.
The resulting symmetric positive-definite matrix has `n^2` rows and
`5 n^2 - 4 n` nonzeros. Columns are strictly increasing within every row.

The dense input is deterministic: entry `i` is
`1 + (i mod 97) / 97`. No model, mesh, or backend-selection semantics are
inferred from this performance-only algebraic fixture.
