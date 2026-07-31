# Model source

This case deliberately reuses the exact direct model from
[`fixed-reference-monolithic-step-2d`](../../fixed-reference-monolithic-step-2d/models/direct.eqi).
The recorded bundle retains its historical v4 Model bytes unchanged. The
current runtime does not decode them; host replay lowers the separately frozen
current Model bridge with the same generation-v2 structural fingerprint, so no
second copied source can drift from the owning FSI fixture and no historical
Run is relabelled.
