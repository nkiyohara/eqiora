# Expected observations

Q1 FEM and TPFA FVM complete-Field JVPs agree with independently recompiled
centered finite differences. VJPs agree componentwise with the same oracle and
satisfy the JVP/VJP duality pairing. FEM boundary outputs retain the exact
direct `boundary_offset` tangent.

Primal, JVP, and VJP expose the same accepted Field. Evidence records normal
orientation for primal/JVP, transposed orientation for VJP, analytic assembled
actions, one exact state-system fingerprint, and an accepted primal residual.
Foreign identities and inadmissible arrays fail before derivative publication.
Complete DLPack producers negotiate a no-copy CPU:0 protocol view for both JVP
and VJP, then cross the same descriptor gate into one Eqiora-owned staging
copy. Incomplete producers, foreign device identities, and invalid
dtype/rank/shape/stride/value payloads fail before derivative publication.
