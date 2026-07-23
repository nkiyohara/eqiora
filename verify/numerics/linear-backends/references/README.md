# Reference provenance

The small deterministic Eqiora CG implementation is an independent oracle for
the SPD path. The nonsymmetric solution is manufactured exactly. faer 0.24.4
provides the production CG and BiCGSTAB algorithms behind the isolated adapter:

- <https://docs.rs/faer/0.24.4/faer/matrix_free/conjugate_gradient/>
- <https://docs.rs/faer/0.24.4/faer/matrix_free/bicgstab/>

Eqiora recomputes the accepted true residual independently of faer's recursive
residual estimate.
