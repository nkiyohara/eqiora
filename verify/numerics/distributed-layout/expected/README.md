# Acceptance

For one, two, and four partitions, every gathered output value must exactly
match the direct global CSR action. Every global entry has one owner; ghosts
are sorted, unique, disjoint from owned entries, and sourced from the declared
owner. A reproducible dot product is reduced from unique-owner contributions
in partition order; unsupported fast reduction fails closed. Invalid partition
counts and malformed CSR data fail before execution.
