# Evidence acceptance contract

Public collection creates:

- `observations/repetitions.csv` with every raw CPU and CUDA phase sample;
- `observations/environment.json` with a clean public source commit and the
  privacy-bounded environment fields described by the case README; and
- `expected/summary.json` with medians and the observed crossing outcome.

Before registration, the host-only replay must reject missing repetitions,
inconsistent shape or byte counts, failed oracle fields, incorrect medians,
forged crossing decisions, malformed source provenance, or identifying
environment fields. Raw repetitions remain the source of the derived summary.
Frozen timing values are observations, not performance regression limits or
production backend-selection policy.
