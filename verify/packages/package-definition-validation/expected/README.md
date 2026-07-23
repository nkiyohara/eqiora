# Expected evidence

`contract.json` freezes the successful package semantic/source identity and
one stable diagnostic code plus message fragment for every negative fixture.
The integration test additionally requires a real source span and forbids a
`GraphPath` on every definition-time diagnostic.

Identity changes require review of the package digest domain. Diagnostic
changes require review of the shared source/semantic typing contract; this is
not a snapshot to refresh mechanically.
