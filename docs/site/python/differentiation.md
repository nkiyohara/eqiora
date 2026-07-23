# Differentiation and framework adapters

Eqiora differentiates an accepted implicit relation through primal, JVP, and
VJP contracts. Python receives an opaque `DifferentiableProgram` bound to
exact Model, Realization, input Parameter, and output Field identities.

The first alpha verifies one host-CPU rank-one `float64` scalar-elliptic path.
The optional PyTorch adapter uses the accepted VJP in first-order backward;
the optional JAX adapter uses typed native CPU FFI for primal, JVP, and VJP.
Base `import eqiora` imports neither framework, and no device transfer is
hidden.

Read the maintained
[differentiation contract](https://github.com/nkiyohara/eqiora/blob/main/docs/python/differentiation.md)
for exact versions, examples, transformations, and explicit nonclaims.
