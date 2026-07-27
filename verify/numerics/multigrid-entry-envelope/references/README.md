# References

The declared theory is the standard conditioning result for a uniformly refined
second-order elliptic operator: the condition number grows as `O(h^-2)`, so
unpreconditioned conjugate gradients need `O(h^-1)` iterations, giving slope 1
and ratio 2 under uniform halving. A method that scales holds slope 0 and
ratio 1.

The predicates in `case.toml` are stated against that asymptotic rather than
against any published table, so no external reference is required to evaluate
them.
