# Differentiation and framework adapters

Eqiora differentiates an accepted implicit relation through primal, JVP, and
VJP contracts. Python receives an opaque `DifferentiableProgram` bound to
exact common Plan, input Parameter, and output Field identities.

The first alpha verifies one supplied-rectangle 2D host-CPU rank-one `float64`
scalar-elliptic common-Plan path.
The optional PyTorch adapter uses the accepted VJP in first-order backward;
the optional JAX adapter uses typed native CPU FFI for primal, JVP, and VJP.
Base `import eqiora` imports neither framework, and no device transfer is
hidden.

The maintained
[differentiation guide](https://github.com/nkiyohara/eqiora/blob/main/docs/python/differentiation.md)
lists supported versions, transformations, and complete examples.
