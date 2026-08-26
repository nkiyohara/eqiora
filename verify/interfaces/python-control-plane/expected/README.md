# Expected observations

The executable integration test owns the expected observations because Model
identities and digests are derived from the single artifact created during the
test. It compares that artifact with its exact Rust replay and checks only
invariants that remain meaningful without freezing a freshly minted identity.

No whole-file digest or exact-byte expectation exists for the Python package
root or stub. Their compile claim is the runtime call shape and the matching
structural stub declaration; additive or otherwise unrelated root API changes
are outside this case.

Expected failure families are `compatibility`, `validation`, `execution`, and
`internal`. Malformed exact wire carries `EQ0901`, a stale edit carries
`EQ0106`, invalid execution policy carries `EQ0501`, and the contained panic
carries `EQ0002`. The panic probe subprocess emits neither its private payload
nor its Rust source location on stderr. Two identical graph-local transactions
over divergent base artifacts have different exact-plan keys and compare
unequal.
