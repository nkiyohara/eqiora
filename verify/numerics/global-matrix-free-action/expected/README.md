# Expected results

The nonsymmetric fixture must produce exactly

```text
A = [[59,  0, 53],
     [71,  0, 67],
     [47, 81,  0]]

b = [170.5, 192.5, -17]
A [2, -3, 5]^T = [383, 477, -149]^T
A^T [7, 11, -13]^T = [583, -1053, 1108]^T
```

Both sides of the adjoint identity equal `9865` exactly.

For Cartesian Q1 in dimensions one through three, packet and CSR action and
transpose differ by at most `3e-13`; their RHS values are identical. Reference
CG solutions differ by at most `5e-12`, the independently replayed CSR residual
is at most `2e-12`, and every free value differs from the affine exact solution
by at most `2e-12`.
