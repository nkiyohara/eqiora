# Acceptance

At one, two, and four physical MPI ranks on one host:

- the authenticated mesh revision and exact cell ownership derive the admitted
  layout;
- only a cell's owner evaluates its canonical packet;
- MPI transports the complete checked row-route inventory to equation-support
  row owners;
- owner reduction preserves target and ascending global-packet order;
- gathered owner shards reconstruct exactly two complete target systems;
- reduced and full CSR indices, matrix bits, and RHS bits equal an independent
  complete CPU reference assembly;
- the reduced canonical CSR fingerprint equals the reference fingerprint;
- the common receipt records eight packets, two targets, and the exact rank
  count; and
- serial-host MINRES passes the unchanged coupled FSI acceptance checks.

At two and four ranks, a layout whose mesh revision differs on exactly one rank
must return the same `EQ0806` diagnostic everywhere before variable-size
collectives. A subsequent all-gather must complete within the parent's bounded
timeout.
