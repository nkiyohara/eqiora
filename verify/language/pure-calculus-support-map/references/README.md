# References

The normative design is [RFC 0056](../../../../rfcs/0056-pure-calculus-and-support-maps.md).
The calculus receives shapes, frames, dimensions, and exact supports only from
the existing identity-parametric Kernel typing contract.

The implementation is intentionally narrower than UFL or a universal weak
form language. It follows the structure-preserving separation used by UFL and
TSFC, a closed transformable subset in the spirit of MLIR Linalg, capture-free
operator boundaries familiar from Modelica, and the separation between mesh
support metadata and numerical transfer operators in PETSc/MFEM.

Exact proof equality is not floating-point program equality. Ordered execution
continues through component scalarization and the shared scalar SSA evaluator.
Component rows use typed local input slots; those slots preserve exact source
coordinates without masquerading as Semantic Parameters.
