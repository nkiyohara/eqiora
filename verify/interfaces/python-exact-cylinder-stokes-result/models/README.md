# Input Model

The only admitted input is a byte-exact installed-package copy of the
repository's sole canonical
`examples/steady-flow-past-cylinder.model.json`. It is generated into the
wheel at `eqiora/examples/steady-flow-past-cylinder.model.json` and supplied
explicitly to the bounded solve. Its canonical 16,797-byte payload has raw
SHA-256
`672016cb80683fb1448adab79d7c8f6a2fdda22f92c6df2d82b684bd5e65e099`.
The packaged file adds one terminal newline, giving 16,798 bytes and raw
SHA-256
`5c5c7924d6efe624a4b4df5f03f2fab03e423fc2ebafb658ba8ad050a7496387`.
Its independently derived current artifact digest is
`8bc5155bc1b64ed37f7a2ac010a966e1619091a118e6cf7806dbdf9621977146`
at semantic revision 1 and accepted source revision 1.

JSON whitespace is not identity. The current Model envelope decoder
canonicalizes valid JSON, so compact and pretty encodings of this exact
document must replay to the same Result. A different source revision remains
outside this frozen application contract even when its semantic digest is
unchanged. This one resource creates no general packaged-model catalogue,
discovery protocol, or loader surface.
