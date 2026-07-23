# Model

The installed-wheel case constructs the same bounded canonical two-dimensional
Poisson model used by the framework-neutral differentiation slice. Its ordered
runtime coordinates are `source_scale`, `diffusion`, and `boundary_offset`;
`wave_number` remains frozen in the exact Model.

The executable source lives in `bindings/python/tests/test_jax.py` so the case
crosses the installed-wheel package boundary, JAX tracing and lowering, and the
native typed-FFI registration seam for both Q1 FEM and TPFA FVM.
