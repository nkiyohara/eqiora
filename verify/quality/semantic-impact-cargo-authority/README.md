# Repository-derived bounded Cargo authority

This case verifies the private Cargo-only S1 analysis at exact C1_HEAD commit
`8e9aec96d5170cb7ab7b7a5f52281e0a0ef09582` and tree
`9994b7e1414447d9781f53608032c93f48cb91d6`. Its independent oracle is the
accepted Cargo grammar and the accepted nine-array seal, not production output.

The ordered test first compares complete typed results for the unchanged commit,
the two-profile proc-macro overlay, the proc-macro-plus-binary overlay, the
six-target matrix overlay, and the five-target `autolib=false` overlay. Equality
includes every Manifest, Lock, Package, Target, and Dependency record; Base and
Head revision identity; per-profile CargoManifestGraph certificate; `Complete`;
and exact empty `precise_cases` and `unknowns` sets.

It next proves that two independently valid Added carriers reach their intended
structural terminal: absent required proc-macro root yields
`RequiredCoverageMissing`, while two integration-test roots for one identity
yield `AuthorityConflict`. Neither failure invents records, a certificate, an
Unknown, or a partial result.

The test embeds and authenticates all nine LF-terminated JSONL streams by exact
record count, byte length, and SHA-256 before parsing them into the private typed
constructors. It independently replays the six Added-only carriers and their
exact lengths and SHA-256 identities. Literal byte identities include the
120-byte proc-macro manifest, one LF, `fn main()`, the 265-byte target manifest,
and its 258-byte no-library form. The unchanged commit raw diff is 0 bytes with
the SHA-256 of empty input; each exact C1_HEAD tree listing has 2,149 records,
243,830 bytes, and SHA-256
`16f22eb7c5a6efafca7a0a75b15c45d051d94787c27e5bdf7c487bf4a085b618`.

For every consumed S1 cap, the exact product succeeds unchanged and its
one-smaller and zero controls fail with `CapBeforeSafeFallback` (except the
genuinely empty commit raw diff, which succeeds at zero). Carrier controls cover
all six overlays. Field 18 uses persistent per-side sums: unchanged commit
577,363; proc-macro 662,914; proc-macro-plus-binary 335,071; six-target 339,005;
and autolib-off 338,031. The two-profile proc-macro request additionally rejects
334,083, the largest individual stream, proving profiles are not reset or
reduced to a maximum. There is no aggregate stream or aggregate digest.

After the ordinary and terminal paths, typed whole-result mutations prove that
the oracle rejects a missing proc-macro root, an accidental library sibling,
dash-normalization drift, missing or second default roots, implicit/explicit
double counting, six-to-five target drift, omitted build script, ambiguous
integration root, suffix/name manifest proxies, content-identity mismatch,
zero or multiple matching manifest authorities, a generic Unknown substitute,
and treating the unused autolib-off `src/lib.rs` as a target.

This is not S2 selection and produces no CaseId result or explanation. It makes
no endpoint, reference, module, facade, general change-classification, external
resolution, target-specific-cfg, Modified/Deleted, Cargo-output, resolver-3
build-unit, portability, performance, public API, compatibility, persisted
wire, aggregate artifact, filesystem-materialization, ambient-config, network,
host-path, sandbox, or new OS-trust claim.
