# Problems

The integration slice reuses the canonical Poisson source in
`verify/numerics/poisson-fem-fvm/models/poisson.eqi`. Adapter unit evidence also
uses the exact systems

```text
[4 1; 1 3] x = [1, 2]       (SPD)
[4 1; 2 3] x = [2, -4]      (nonsymmetric; x = [1, -2])
```

These systems are execution evidence only and do not introduce canonical
model nodes.
