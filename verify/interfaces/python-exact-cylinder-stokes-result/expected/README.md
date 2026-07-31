# Frozen observations

The installed-wheel test embeds the accepted model, exact-source, and mesh
digests; six independently agreed pressure probes; signed inlet and outlet
flux; cylinder constraint force on the fluid; global force balance; and the
frozen faer sparse-LU solve plan.

Generated realization, correspondence, snapshot, and run identities are not
literal expected values. The test derives them relationally from closed
canonical public documents. Pressure extrema, full-field hashes, timings, and
exact residual values are likewise not frozen.

The tolerances are imported unchanged from the scientific case:

- pressure: `2e-14 + 5e-7 * (0.001 * 0.3 / 0.41)`;
- flux: `2e-13 + 5e-7 * (0.3 * 0.41)`;
- reaction: `2e-14 + 5e-7 * (0.001 * 0.3)`; and
- solver true-residual target: `1.3239627651209673e-7`.
