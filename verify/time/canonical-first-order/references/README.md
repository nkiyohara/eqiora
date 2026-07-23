# References

The expected ODE and DAE trajectories follow by direct integration and
constraint elimination from `models/problem.md`; they are independent of the
implementation and backend.

The backend behavior is documented by the
[Diffsol 0.16.1 API](https://docs.rs/diffsol/0.16.1/diffsol/). The test uses
Diffsol only after Eqiora has completed structural equation classification and
normalization.
