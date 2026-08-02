# Frozen observations

No image baseline, solver value, extremum, or tolerance is frozen here.

The only frozen data are already accepted structural facts, wired verbatim
from the registered trajectory evidence and re-derived inside the test from
that same connectivity:

- the 8-triangle reference connectivity of the admitted 2D trajectory;
- the accepted vertex support membership `fluid_pressure = [0, 1, 2, 3, 4, 5]`
  and `solid_displacement = [1, 3, 5, 6, 7, 8]`;
- the cells whose complete vertex tuple lies in that support,
  `fluid_pressure = [0, 1, 2, 3]` and `solid_displacement = [4, 5, 6, 7]`; and
- the nine sorted unique undirected edges of the admitted solid cells,
  `(1,3) (1,6) (1,7) (3,5) (3,7) (3,8) (5,8) (6,7) (7,8)`.

Every numerical field value, coordinate, extremum, and digest is read from the
live accepted trajectory at run time and compared to itself under the exact
relations in [`../case.toml`](../case.toml). Scalar color limits are the
extrema of the support-restricted values; deformed positions are
`coordinates + scale * values`.

The support-restricted-limits falsifier is discriminating whenever the accepted
extrema of the admitted values exclude zero, because the outside-support
entries are exactly `+0.0`. The oracle asserts the exact support-restricted
limits unconditionally and pins the relation
`whole-block extrema == (min(support_min, 0.0), max(support_max, 0.0))`; it
assumes no sign for the accepted pressure and freezes no pressure value.

The output oracle checks only that a real headless canvas draws and that the
caller can encode a valid, decodable, nonblank PNG. PNG bytes, pixels,
dimensions, compression, metadata, fonts, colormaps, and layout metrics remain
unfrozen.
