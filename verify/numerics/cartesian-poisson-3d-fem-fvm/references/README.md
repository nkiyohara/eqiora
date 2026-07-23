# Reference provenance

The exact solution and forcing are derived directly in
[`models/problem.md`](../models/problem.md); no external table or third-party
dataset is used.

`expected/convergence.csv` records the deterministic reference-backend output
after compiling [`models/poisson.eqi`](../models/poisson.eqi), including the
fixed-order inner-product tree specified by RFC 0017. CI compares the stored
values and also enforces independent monotonicity, order, and balance
thresholds. The table is regression evidence, not a second semantic model.
