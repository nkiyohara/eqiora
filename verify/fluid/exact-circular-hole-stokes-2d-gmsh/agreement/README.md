# Route agreement

`check.py` reads the two frozen route records without running either solver.
It fails closed on the accepted source citation, GEO/MSH and Eqiora mesh
digests, mesh counts, boundary partition, selector names, selector positions,
and tie multiplicities. It then compares every shared velocity probe, pressure
probe and extremum, signed flux, cylinder reaction, and momentum closure under
the pre-existing route tolerances. Each route's residual and unchanged closure
bounds must pass before the differences are reported.

The checker deliberately contains no solver, mesh parser, alternative expected
table, tolerance fitting, or implementation-derived value.
