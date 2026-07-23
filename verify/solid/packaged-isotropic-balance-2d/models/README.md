# Models

The ordinary dependency is the immutable `Eqiora.Solid.LinearElasticity@0.1.0`
release owned by this conformance root. `components.eqi` and
`components-permuted.eqi` repeat its one Component only as synthetic
provider-name and declaration/file-order falsifiers; they are not a second
canonical library source.

`manufactured.eqi` is the root package source. It owns the body, four sides,
Fields, four Parameters, load definition, and boundary closure, and binds the
dependency Component explicitly. The two Lamé bindings forward the root
Parameter identities. `manufactured-permuted.eqi` changes declaration and
binding order without changing meaning. Tests substitute only the resolved
dependency alias; neither fixture contains a package registry or execution
dispatch key.

`linear-load.eqi` is a second root over the same exact dependency. Its affine
potential yields the nonzero integrated body force used to falsify hidden
package-boundary load or reaction errors.

The explicit-flat oracles remain the already registered manufactured and
linear-load fixtures in `solid.isotropic-elasticity-2d`. They are not copied
here.
