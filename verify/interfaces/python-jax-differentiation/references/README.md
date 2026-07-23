# References

- The native primal/JVP/VJP reference is
  [`interfaces.python-differentiation`](../../python-differentiation/README.md).
- JAX's typed FFI path is documented in the
  [JAX FFI guide](https://docs.jax.dev/en/latest/ffi.html).
- The independent JVP/VJP transformation seam follows the
  [JAX custom-derivatives guide](https://docs.jax.dev/en/latest/hijax_custom_derivatives.html).
- The gate compiles a C layout probe against the exact JAXLIB 0.11.0
  `xla/ffi/api/c_api.h` installed beside the tested wheel.
