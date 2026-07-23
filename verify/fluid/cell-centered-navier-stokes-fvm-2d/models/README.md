# Model

The authoritative fixture is
[`../../fixed-domain-transient-navier-stokes-2d/models/direct.eqi`](../../fixed-domain-transient-navier-stokes-2d/models/direct.eqi).
The integration test includes those exact bytes and compiles them once for the
FVM path. This directory intentionally contains no copy: duplicating the
source would weaken the same-Model claim and create a drift surface.
