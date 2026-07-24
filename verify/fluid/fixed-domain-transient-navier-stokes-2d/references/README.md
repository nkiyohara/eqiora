# Independent references

The canonical conservative relation is compared with its explicitly selected
energy-skew MINI weak realization. Their globally assembled defect is checked
against the exact divergence consistency term. Every analytic Jacobian column
is independently checked by centered differences of a directly assembled
nonlinear residual. Conservative structural coloring changes only how many
complete residual pairs are shared: the topology-, constraint-, and
quotient-derived pattern reconstructs every individual column and never reads
analytic Jacobian values. Backward-Euler order is checked in the consistent MINI
mass norm by fixed-mesh step doubling at one common physical final time.
A tenfold tighter nonlinear solve is compared in that same norm to bound
accumulated nonlinear-solve sensitivity without identifying a weak residual
norm with a state-error norm.
