# Unstructured affine-simplex shape and compliance verification

This case realizes one canonical rectangular Poisson model on a
fixed-connectivity triangular mesh without Cartesian mesh indexing. The mesh
has perturbed interior vertices, alternating cell diagonals, and positively
oriented affine cell maps. Construction fails before assembly if a cell is
inverted, falls below the declared mean-ratio threshold, or belongs to a
non-manifold facet.

The selected canonical Domain bounds remain model meaning. A realization-local
map turns each selected coordinate into one velocity per mesh vertex while
preserving normalized box coordinates. P1 FEM consumes only the resulting
affine geometry-map JVP; vertex IDs do not enter the canonical coordinate
space.

The same lowering produces the residual relation and the quadrature-defined
continuous-field compliance

```text
J_h = integral_Omega source(x, p) u_h(x, p) dx.
```

Its value, state cotangent, and direct design cotangent form one typed objective
linearization. Forward state sensitivities and adjoint compliance gradients are
compared with independently compiled, rebuilt, quality-checked, and solved
positive/negative Domain revisions. A refinement sequence also compares the
discrete compliance with the independent Fourier-sine series for the continuous
rectangular problem.

This is fixed-topology, discretize-then-differentiate evidence. Positive
orientation proves cell-local affine injectivity only; it does not prove global
mesh non-overlap. The case does not claim continuous Hadamard shape calculus,
remeshing, adaptivity, topology changes, high-order geometry, or a production
optimizer.
