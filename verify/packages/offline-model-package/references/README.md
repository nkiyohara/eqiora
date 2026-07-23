# References

- [RFC 0021](../../../../rfcs/0021-component-hierarchy-and-instantiation.md)
  defines deterministic component elaboration into the flat kernel.
- [RFC 0022](../../../../rfcs/0022-exact-package-identity-and-resolution.md)
  defines exact Model Package identity, source identity, resolution, and
  provenance boundaries.
- [RFC 0027](../../../../rfcs/0027-capability-rooted-package-directory-admission.md)
  defines explicit directory authority, no-follow admission, resource bounds,
  and the multi-file snapshot nonclaim.
- [RFC 0028](../../../../rfcs/0028-retained-local-package-store-replay.md)
  defines explicit read-only store authority, exact digest entry reads, bounded
  nonblocking I/O, and durable lock replay.
- [RFC 0029](../../../../rfcs/0029-atomic-package-store-installation.md)
  defines separate write authority, synchronized same-directory staging, and
  atomic no-clobber publication.
- [Rust `hard_link`](https://doc.rust-lang.org/std/fs/fn.hard_link.html)
  documents the existing-destination failure and platform primitives used by
  the bounded publication seam.
- [`language.component-elaboration`](../../../language/component-elaboration/README.md)
  is the local-source hierarchy and explicit-flat semantic baseline.
