# Host reference global matrix-free action

This case closes the host-local boundary between entity-local operator data
and a complete solver action without materializing global CSR inside that
action.

`PacketLinearSystem` evaluates one ordered `AssemblyWork`, projects every
selected target through the existing `AssemblyPacket::project` contract, and
retains packet-local mapped rows plus the constraint-aware right-hand side. A
temporary coordinate accumulator applies the same exact-zero structural-row
gate as reference assembly and is then discarded; no global CSR is retained
by the packet system. The normal action, row action, transpose, and diagonal
are all derived from this same immutable packet projection. Fixed columns
never become part of the linear action; their affine contribution appears only
in the RHS.

The first oracle is a three-packet nonsymmetric integer system calculated by
hand. It contains duplicate rows and free columns, a skipped equation, two
nonzero fixed values of opposite sign, and cross-packet scatter. This prevents
the independently assembled CSR and packet action from validating a shared
projection bug merely by agreeing with each other.

The spatial path then lowers and executes the existing Cartesian Q1 diffusion
local action in one through three dimensions. The same immutable packet
producer separately feeds the packet system and reference CSR assembly. Their
normal action, transpose, diagonal, and constraint-aware RHS are compared
before reference CG solves the packet operator. The CSR action independently
checks the final residual, and the free values are compared with the exactly
representable affine harmonic solution.

The operator stores mapped packet contributions and can therefore use more
memory than a compressed matrix for low-order problems. This case establishes
meaning and executable composition, not performance, global-CSR-free canonical
Realization, threading, distribution, or accelerator support.
