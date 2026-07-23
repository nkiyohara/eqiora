# Acceptance criteria

- Checkpoint canonical bytes and domain-separated digest survive decode and
  re-encode exactly for nontrivial finite floating-point values.
- Canonical residual replay remains below `1e-12` and agrees with the recorded
  replay norm.
- Parent output, checkpoint, child initial data, child start time, and child run
  form one acyclic, content-addressed restart edge.
- Restarted and uninterrupted terminal states agree within `1e-14`.
- Value drift, dimension excess, missing output linkage, start-time drift, and
  a parent/child cycle fail closed with a structured artifact diagnostic.
