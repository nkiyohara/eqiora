# Acceptance

The executable test owns all numerical tolerances. It requires two accepted
nonzero steps, separate momentum/mass/gauge acceptance, replay and global-mass
defects within their derived floating-point bound, centered-JVP defect below
`2e-7`, affine pressure correction below `1e-12`, nonzero checkerboard action,
physical scale/reflection agreement below `2e-8`, and a BDF1 step-doubling
ratio between `1.6` and `2.4` on one fixed mesh.

No golden coefficient vector is stored: physical cell fields, exact lineage,
structural falsifiers, and independently reconstructed actions are the stable
evidence boundary.
