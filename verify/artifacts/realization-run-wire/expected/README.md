# Frozen Realization wire fixtures

- `realization-v1.json`: scalar compatibility envelope
- `realization-v2.json`: field-wise compatibility envelope
- `realization-v3.json`: coupled multi-Domain compatibility envelope

Each file is exact canonical JSON followed by one repository text newline. The
wire tests remove only that newline, decode with the corresponding exact
version, and require canonical re-encoding to reproduce every committed byte.
