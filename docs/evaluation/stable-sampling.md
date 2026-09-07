# Stable sampled Parameter inputs

`ParameterSampler` freezes real inputs for an existing `DifferentiableProgram`.
Clients supply a master stream, a full sample ID and a channel role. They pass
the resulting `point().values()` explicitly to `program.evaluate(...)` or an
`EvaluationMapPlan`. Sampling does not modify the Program identity or happen
inside evaluation, JVP, VJP or diagnostics.

```rust,ignore
let sampler = ParameterSampler::new(
    program.clone(), SamplingGenerator::Sha256UniformV1,
    [42; 32], &[[1.0, 2.0]], // coherent-SI bounds in the exact input order
)?;
let request = SamplingIdentity::new(
    b"replicate-17", b"diffusion", &[batch_number, request_position],
    SamplingCoupling::IndependentReplicate,
)?;
let frozen = sampler.sample(&request)?;
let accepted = program.evaluate(frozen.point().values())?;
```

The ordinary `stable_sampling` integration test executes this client path with
the existing typed Poisson Program and also passes sampled points to mapped
evaluation. Finite input admission precedes physical/numerical acceptance: a
draw can still fail the Program's usual checks.

## Identity and replay

Occurrence lineage is retained association metadata, not random-key material.
Permuting requests, changing chunk positions, retrying work or changing worker
counts therefore preserves the sample's draws. Equal numerical points remain
distinct requests; their values never determine their identities.

Independent-replicate requests derive from their complete sample IDs.
`CommonRandomNumbers(coupling_id)` explicitly substitutes a common coupling ID
in the stream key while retaining each request's sample ID. Equal coupling IDs
and channels under the same master intentionally share draws. Distinct IDs are
not statistical evidence of independence.

## SHA-256 uniform V1

This version's stream digest is SHA-256 of the following concatenation:

1. ASCII `eqiora.parameter-uniform.sha256.v1` and a zero byte;
2. all 32 master bytes;
3. the channel's byte length as unsigned 16-bit big-endian, then all its bytes;
4. byte `0` for independent replication or `1` for common random numbers;
5. the effective sample/coupling ID length as unsigned 16-bit big-endian, then
   all its bytes.

IDs are nonempty byte strings of at most 256 bytes. Occurrence lineage contains
at most 32 unsigned 64-bit entries. All request fields and all 256 stream-digest
bits are retained; identity is never reduced to floating-point precision.
SHA-256 provides a collision-resistant derivation, not a proof that collisions
are mathematically impossible.

Coordinate `i` hashes ASCII `eqiora.parameter-uniform.draw.v1`, a zero byte,
the full stream digest and unsigned 64-bit big-endian `i`. Interpret the first
eight digest bytes as big-endian `u64`, shift right by 11, and multiply the
exact resulting integer by `2^-53` to obtain `u` in `[0, 1)`.

Each of 1–256 ordered intervals requires finite `lower < upper` and finite
`upper - lower`. Conversion uses separate binary64 subtraction, multiplication
and addition: `lower + ((upper - lower) * u)`. If rounding reaches `upper`,
return `upper.next_down()`. Thus the result lies in `[lower, upper)` even for
adjacent or subnormal endpoints. This is a finite-grid conversion, not an
assertion of a continuous probability density. Reordering Program input
coordinates changes which coordinate draw is assigned to each Parameter.

The ordinary private fixed-ID test contains independently calculated Python
`hashlib` references for this exact encoding, plus identity-separation and
rounding falsifiers. No cross-generator-version/device guarantee or
time-indexed Wiener/jump process is implied.
