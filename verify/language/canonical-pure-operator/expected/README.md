# Acceptance

The registered target must prove:

- direct source variants differing only in file path, declaration order,
  field order, and formal spelling compile to identical current Model bytes;
- exact package variants differing only in source location, declaration
  order, formal spelling, and dependency alias retain identical package
  semantic identities and current Model bytes;
- local and package-resolved calls retain one content-addressed definition
  identity and lower to exactly one generic application, with no `dyadic`
  name in canonical Model bytes;
- the current Model and Transaction replay exactly through the one current
  owner;
- the source-compiled definition scalarizes to `[10, 14, 15, 21]`; and
- unknown required features, forged definition digests, and selected decoder
  resource limits fail closed.

No numerical field, discretization, weak form, solver, device, schedule,
callback, or floating-point rewrite is expected from this case.
