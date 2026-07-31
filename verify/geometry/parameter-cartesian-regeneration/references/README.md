# Reference provenance

The exact target and partial-update values in `expected/oracle.json` were
precommitted by two independent derivations from the public coordinate recipe:

- an analytic rational-arithmetic route derived endpoint changes, widths,
  volume, and the coupling residual; and
- an independent symbolic/numerical route recomputed interval products and the
  width-sum/width-difference checksum.

Both derivations used only
`x = [-1, p]`, `y = [p, 6]`, `z = [0.5, 5.5]`, and `p: 2 -> 3.5`.
Neither inspected implementation code or an existing fixture. All admitted
numbers are dyadic rationals, so exact binary64 equality is the oracle; no
tolerance was selected or tuned by the implementation lane.

Geometry Identity and selection retention are not scientific derivations.
They replay existing accepted artifact contracts over the independently
checked source and target bounds.
