# Reference provenance

The acceptance contract was derived independently before implementation.

- `fsi.fixed-reference-monolithic-step-2d` owns the scientific one-step
  coefficients, stopping evidence, and physical acceptance.
- `artifacts.fixed-reference-fsi-spatial-trajectory` owns the two-state,
  trajectory, and final Run lineage.
- The common Python `Result`, `Trajectory`, typed FSI evidence, and Matplotlib
  cases supply the application, installed-wheel, immutable-array,
  optional-dependency, and headless-Figure contracts.

The registered Rust scientific test compares both accepted states under both
automatic-default and complete manual scaling from the common `Plan` path
directly with its unchanged independent support composition for vertex
velocity, MINI bubbles, pressure, and displacement at the existing bound.
Separately, the installed Python common-worker test proves its own
caller-selected output order, exact snapshot Field identity, and Run request to
Plan, initial State, output State, and Trajectory lineage. No lineage digest is
compared with the independent scientific support composition. This case adds no
scientific expected value, tolerance, or falsifier.
