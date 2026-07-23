# Model

The installed-wheel case constructs the same bounded canonical two-dimensional
Poisson model used by the framework-neutral differentiation slice. Its ordered
runtime coordinates are `source_scale`, `diffusion`, and `boundary_offset`;
`wave_number` remains frozen in the exact Model.

The executable source lives in
`bindings/python/tests/test_torch.py` so the test exercises the installed-wheel
package boundary: the public adapter plus its private registration seam for
`opcheck` and token falsifiers.
