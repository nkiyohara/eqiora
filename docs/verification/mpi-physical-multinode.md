# Physical multi-node MPI verification

This hardware-gated acceptance case graduates functional transport and solve
correctness across two physical nodes, distinct from the ordinary one-host
multi-process CI case. It reuses the same distributed layout, halo,
collective, solver-plan, and true-residual contracts; no second solver or
MPI-shaped model API is introduced.

## Contract under test

```text
one validated global CSR problem
        ↓ balanced owned/ghost partition
one MPI rank per physical node
        ↓ exact MPI_Get_processor_name all-gather
minimum distinct-processor precondition
        ↓
halo action + reproducible/native dot products
        ↓
Jacobi-CG under the sole SolverPlan
        ↓
fresh distributed true-residual acceptance
```

The integration-test child normally makes no physical-topology claim. Setting
`EQIORA_MPI_MIN_PHYSICAL_NODES` opts into a bounded runtime precondition. Every
rank obtains its processor name through MPI, all ranks gather the complete
fixed-size byte representation, and the test rejects a launcher placement with
fewer distinct processors than requested. The requested count must be positive
and cannot exceed the communicator rank count.

Processor names are verification inputs only. They do not enter Semantic
Model, Realization, distributed layout identity, solver reports, or run wire.
The stable run provenance continues to record MPI implementation/version,
thread support, rank topology, and reduction policy without making deployment
hostnames part of portable artifact identity.

## Hardware command

Build the integration test with the system MPI used on every allocated node:

```bash
cargo test --locked -p eqiora-backend-mpi \
  --features mpi-runtime --test mpi_distributed --no-run
```

Then launch the printed `mpi_distributed-*` executable with one rank on each of
at least two physical nodes. Exact launcher flags are scheduler-specific. For
Open MPI, explicitly export the verification inputs to the remote ranks:

```bash
export EQIORA_MPI_TEST_CHILD=1
export EQIORA_MPI_MIN_PHYSICAL_NODES=2
mpirun --map-by ppr:1:node -n 2 \
  -x EQIORA_MPI_TEST_CHILD -x EQIORA_MPI_MIN_PHYSICAL_NODES \
  /path/to/mpi_distributed-HASH \
  --exact mpi_child_executes_halo_and_collectives --nocapture
```

For MPICH/Hydra, the equivalent explicit placement and propagation are:

```bash
mpirun -hosts node-a,node-b -ppn 1 -n 2 \
  -genv EQIORA_MPI_TEST_CHILD 1 \
  -genv EQIORA_MPI_MIN_PHYSICAL_NODES 2 \
  /path/to/mpi_distributed-HASH \
  --exact mpi_child_executes_halo_and_collectives --nocapture
```

Both environment values must be exported to remote ranks. The executable and
its dynamic MPI/runtime libraries must resolve to the same builds on all
nodes. The hardware launcher is deliberately outside ordinary CI; the default
test still runs 1/2/4 ranks on one host and proves protocol behavior without
claiming physical distribution.

The acceptance gate has passed with two ranks on two distinct physical nodes
using one byte-identical test executable and one MPI runtime build. Its
single-node falsifier also fails before entering the numerical case.
That recorded observation predates the generic admitted complete-CSR bridge
and therefore remains lower-level halo/reduction/CG evidence only. The exact
test name is retained as a compatibility alias to the current child body so
the command does not rot; a new recorded physical run is required before the
generic bridge may claim multi-node evidence.

## Falsifying cases

- zero, malformed, or rank-excessive minimum-node input fails before numerical
  work;
- two or more ranks placed on one processor fail the physical-node
  precondition;
- processor names that exceed the bounded exchange buffer fail closed;
- partition ownership, ghost order, halo values, or rank count drift fails;
- reproducible and native collective reductions must both match the exact
  global fixture;
- both Jacobi-CG runs must match the manufactured solution and satisfy a newly
  recomputed global true residual; and
- MPI initialization/finalization and communicator ownership remain with the
  application/test launcher.

## Nonclaims

This case is evidence for functional execution across at least two physical
nodes. It does not establish strong/weak scaling, NUMA placement, network
topology optimization, distributed assembly, checkpoint/restart, elastic
ranks, process-failure recovery, ULFM, or production scheduler integration.
