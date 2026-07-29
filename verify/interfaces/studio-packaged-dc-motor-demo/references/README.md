# Reference strategy

This application case introduces no new scientific oracle. Its executor is
the existing `packaged_dc_motor_controller` Rust target, whose independently
derived exact sampled transition, refinement checks, dimensioned physical
residuals, and power/energy acceptance remain authoritative.

The interface oracle is structural and independent of those equations:

- exact integer step and commit ledgers;
- closed coherent units and finite values for three named production fields;
- bit-identical voltage within each presented hold interval;
- the exact package closure and mutually linked content identities;
- compile-time attribution to the current registered verified case; and
- fail-closed asynchronous and browser publication.

Studio formats and scales those retained values for display but derives no new
physical quantity from them.
