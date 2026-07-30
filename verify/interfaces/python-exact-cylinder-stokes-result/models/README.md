# Input Model

The only admitted input is the repository's
`examples/steady-flow-past-cylinder.model-v7.json`, supplied by the caller as
bytes. Its independently derived semantic digest is
`668fa55e5ab1a46d0b7523e4e3162442ccd7698697c4308604cf4fe9269249de`
at semantic revision 1 and accepted source revision 1.

JSON whitespace is not identity. The existing ModelEnvelopeV7 decoder
canonicalizes valid JSON, so compact and pretty encodings of this exact
document must replay to the same Result. A different source revision remains
outside this frozen application contract even when its semantic digest is
unchanged.
