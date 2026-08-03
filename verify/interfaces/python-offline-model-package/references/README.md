# References

The Rust package contract and filesystem adversaries remain owned by
`packages.offline-model-package`. Compiler diagnostic identity is obtained by
running the same frozen request directly through
`PackagedModelDocument::compile_locked`, then comparing the Python projection
without changing either result.
