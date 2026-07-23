# Reference oracle

The oracle follows by direct substitution into the canonical momentum and
incompressibility Relations. With zero velocity and force potential
`phi = (1 Pa/m) x`, viscous, inertial, and convective terms vanish and
`grad(p) - grad(phi) = 0`. The zero-integral pressure constraint fixes the
remaining constant, giving `p = x - 0.5 Pa` on the unit square.

The collocated pressure--velocity coupling follows
[Bartholomew et al.](https://doi.org/10.1016/j.jcp.2018.08.030) and
[Denner, Evrard, and van Wachem](https://doi.org/10.1016/j.jcp.2020.109348).
The comparison itself needs no stored external dataset: both discretizations
are tested against the same closed-form physical state.
