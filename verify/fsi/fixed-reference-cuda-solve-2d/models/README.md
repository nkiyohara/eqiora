# Model source

This case deliberately reuses the exact direct model from
[`fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/models/direct.eqi).
The collector persists its canonical `ModelEnvelopeV4`; the host replay lowers
that decoded artifact, so no second copied source can drift from the owning FSI
fixture.
