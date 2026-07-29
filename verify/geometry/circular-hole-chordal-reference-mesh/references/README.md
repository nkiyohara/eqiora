# References

The governing contract for this slice is RFC 0082, which freezes the
circular-boundary rule, the segment-selection inverse, the approximation policy
and its derived evaluation allowance, the reference topology, and the required
falsifiers. That contract and the independent oracle in
[`../oracle.py`](../oracle.py) were frozen before implementation began.

The exact source geometry this realization consumes, and its own independent
identity oracle, are the sibling case
[`../../exact-circular-hole-geometry`](../../exact-circular-hole-geometry/README.md).
That case owns the centre/radius identity; this one owns only its chordal
realization and the approximation evidence.

The high-precision ideal values are reproduced by the oracle itself rather than
quoted from an external table, so no third-party numerical source is evidence
here. Published DFG benchmark outputs are not evidence for this geometry-only
slice: no flow, drag, lift, or Strouhal value is claimed.
