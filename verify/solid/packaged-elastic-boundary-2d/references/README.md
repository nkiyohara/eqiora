# Reference contract

- [RFC 0035](../../../../rfcs/0035-field-valued-boundary-interfaces.md)
  defines nominal trace/flux typing, exact support, frame, outward
  orientation, and Euclidean boundary duality.
- [RFC 0040](../../../../rfcs/0040-occurrence-bound-field-slots.md) defines
  exact occurrence-bound displacement Fields.
- [RFC 0041](../../../../rfcs/0041-complete-exterior-port-families.md) defines
  the complete-exterior obligation and statically elaborated family.
- [RFC 0022](../../../../rfcs/0022-exact-package-identity-and-resolution.md)
  defines exact offline package resolution.

The package source is the immutable `package-v0.2.0` release owned by this
evidence root. Both digest domains are pinned in `case.toml`, and the live
public package must match all three bundled files exactly. A later package
version therefore cannot rewrite accepted `0.2.0` evidence.
