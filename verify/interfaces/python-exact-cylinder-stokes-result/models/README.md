# Input Model

The only admitted input is a byte-exact installed-package copy of the
repository's sole canonical
`examples/steady-flow-past-cylinder.model-v7.json`. It is generated into the
wheel at `eqiora/examples/steady-flow-past-cylinder.model-v7.json` and
supplied explicitly to the bounded solve. Its 16,798 bytes include one
terminal newline and have raw SHA-256
`b6c7be43520070084bf1a0f20a15772a69f4375dce168424341509189ddf5d1f`.
Its independently derived semantic digest is
`668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de`
at semantic revision 1 and accepted source revision 1.

JSON whitespace is not identity. The existing ModelEnvelopeV7 decoder
canonicalizes valid JSON, so compact and pretty encodings of this exact
document must replay to the same Result. A different source revision remains
outside this frozen application contract even when its semantic digest is
unchanged. This one resource creates no general packaged-model catalogue,
discovery protocol, or loader surface.
