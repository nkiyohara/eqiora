# References and oracles

The collocated pressure action and transient face history follow the
time-consistent momentum-weighted interpolation described by
[Bartholomew et al.](https://doi.org/10.1016/j.jcp.2018.08.030) and the fully
coupled formulation of
[Denner, Evrard, and van Wachem](https://doi.org/10.1016/j.jcp.2020.109348).
The checkerboard failure mode and compatible pressure--velocity coupling are
also treated by [Eymard, Herbin, and
Latché](https://doi.org/10.1137/040613081).

The initial velocity is the discrete curl of a Cartesian cell streamfunction.
The two boundary-closed difference matrices commute, so its discrete face-flux
divergence is roundoff zero. Independent oracles are centered finite
differences of every JVP column, retained-face residual replay, affine pressure
null action, alternating pressure action, symmetry transforms, non-unit
normalization, and fixed-final-time step doubling.
