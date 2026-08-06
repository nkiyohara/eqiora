# Model fixtures

`natural.eqi` is the ordinary nonzero positive: one dimensionless Field, one
dimensionless Parameter, and one continuous Relation containing
`lhs = rhs;`. `explicit-residual.eqi` changes only that statement to the
previously accepted `lhs - rhs = 0;` spelling.

The two files are intentionally compiled as fresh occurrences. Their accepted
Models must be structurally equivalent and have the same alpha-normalized
fingerprint, but exact Model and artifact identities are not compared for
equality. Additional zero, underflow, precedence, range, error, package, and
native specimens are bounded inline in the Rust integration test so this case
needs no unlisted fixture or golden path.
