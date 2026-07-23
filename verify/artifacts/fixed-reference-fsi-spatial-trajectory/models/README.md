# Model input

This artifact case deliberately reuses the exact direct fixed-reference FSI
Model registered by `fsi.fixed-reference-monolithic-step-2d`. The Rust evidence
includes that fixture at compile time and executes the same canonical lowering;
this directory adds no second copy whose source bytes could drift.
