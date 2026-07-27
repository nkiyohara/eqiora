# Models

This declaration owns no model of its own. It pins the model that the admitted
measurement must use, by path and SHA-256, in `case.toml`:
`verify/numerics/preconditioner-scaling-envelope/models/constant-source-poisson.eqi`.

Pinning rather than copying keeps one source of truth for the probed problem. A
copy would let the two drift and would make "the same probe" unverifiable.
