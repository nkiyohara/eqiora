# Private fixture boundary

Private tests consume `cfg(test)` probes over the production-private state,
validation, and phase-ledger seams. They do not introduce a second Model or
numerical fixture. The public two-element Model and all scientific values stay
owned by `numerics.faer-sparse-lu-reuse`.
