# References

The semantic oracle is the equivalence-relation normalization contract in
[`RFC 0033`](../../../../rfcs/0033-hierarchical-conserving-connection-sets.md).
The exact-package oracle is the content-addressed release and offline
resolution contract in [`RFC 0022`](../../../../rfcs/0022-exact-package-identity-and-resolution.md).

The numerical oracle is Ohm's law: the 12 V source and 2 ohm branch carry
6 A. Canonical Model equality and solution equality are both required; one
cannot substitute for the other.

The sole nonzero right-hand side is 12 V, hence its Euclidean norm is 12.
The relative solver request is 1e-13 and the absolute request is 1e-14, giving
an accepted solver threshold of 1.2e-12, strictly inside the independently fixed
1e-11 semantic residual oracle. A 1e-12 relative request would permit 1.2e-11
and would not guarantee that oracle. Source-identity changes can permute the
canonical system and alter the iterative stopping point; the physical 6 A
expectation and semantic residual oracle remain unchanged.
