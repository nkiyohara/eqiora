# Reference provenance

The analytic solution and forcing are derived directly in
[`models/problem.md`](../models/problem.md). The independent numerical oracle
is the existing reproducible reference CPU solver applied to the same
canonical model, mesh, and discretization method. It is a comparison path,
not a fallback: the CUDA result must already have CUDA producer evidence and
pass independent host true-residual acceptance.
