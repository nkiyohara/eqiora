# Reference provenance

The reference is the direct, globally resident CSR row action in ascending
row/entry order. The independent path is Eqiora's partition/halo loopback
executor. No MPI implementation participates in this case.

The ownership and ghost vocabulary follows established distributed vector
practice without importing a library's communicator or index-set types into
the Eqiora contract:

- <https://petsc.org/release/manual/vec/>
- <https://docs.rs/mpi/0.8.2/mpi/>
