# Reference provenance

The acceptance contract was derived independently before implementation.

- `fsi.fixed-reference-monolithic-step-2d` owns the scientific one-step
  coefficients, stopping evidence, and physical acceptance.
- `artifacts.fixed-reference-fsi-spatial-trajectory` owns the two-state,
  trajectory, and final Run lineage.
- `interfaces.studio-fixed-reference-fsi-demo` owns the already accepted
  application and presentation boundary.
- The common Python `Result`, `Trajectory`, typed FSI evidence, and Matplotlib
  cases supply installed-wheel, immutable-array, optional-dependency, and
  headless-Figure contracts.

The existing Rust scientific tests compare the production application
composition directly with their independent support composition. This case
adds no scientific expected value, tolerance, or falsifier.
