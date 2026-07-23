# Publication-time configuration evidence

Before this case can advance beyond `specified`, the public repository must
show:

- `CI gate` and `CI definition trust` on representative pull requests;
- both contexts required by the `main` ruleset and bound to GitHub Actions;
- destructive updates and direct pushes rejected;
- a normal product change accepted without a trust-definition bypass;
- a protected-path fixture rejected by the base-owned guard;
- exact-commit manual dispatch still bound to the requested SHA.

Repository unit tests prove the routing predicates and failure modes. They do
not prove live GitHub settings, runner operation, review independence, cost,
or community-scale maturity.
