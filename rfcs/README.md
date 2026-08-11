# Eqiora RFC process

RFCs record changes whose cost or compatibility impact extends beyond one pull
request. They are required for:

- Semantic Kernel additions or meaning changes.
- Standard Ontology schema contracts.
- Public wire formats and stable APIs.
- Dependency-layer exceptions.
- Numerical or differentiation contracts.
- Governance, licensing, or release-policy changes.

## Lifecycle

1. Copy `0000-template.md` to `0000-short-title.md` and open a pull request.
2. Discuss motivation, alternatives, failure modes, and conformance evidence.
3. Revise until maintainers record consensus or a clearly documented
   provisional decision under `GOVERNANCE.md`.
4. After acceptance, assign the next number, merge the RFC, and link its
   implementation issues.
5. Record later reversals in a new RFC; accepted history is immutable.

An RFC is not accepted merely because code exists. Conversely, exploratory
code may be used to answer an RFC's open questions when clearly marked.

## RFC records

The status line inside each RFC is authoritative. This index contains accepted,
partially implemented, and active records; presence here alone is not an
implementation or capability claim.

- [RFC 0001: Executable semantic kernel](0001-executable-semantic-kernel.md)
- [RFC 0002: Reference execution v0](0002-reference-execution-v0.md)
- [RFC 0003: Language frontend v0](0003-language-frontend-v0.md)
- [RFC 0004: Scalar operator IR](0004-scalar-operator-ir.md)
- [RFC 0005: Scalar diffusion realization](0005-scalar-diffusion-realization.md)
- [RFC 0006: Spatial realization contracts](0006-spatial-realization-contracts.md)
- [RFC 0007: Canonical spatial operators](0007-canonical-spatial-operators.md)
- [RFC 0008: Canonical artifact wire v1](0008-canonical-artifact-wire-v1.md)
- [RFC 0009: Realization Graph v0](0009-realization-graph-v0.md)
- [RFC 0010: Execution backend contracts](0010-execution-backend-contracts.md)
- [RFC 0011: Implicit differentiation contracts](0011-implicit-differentiation-contracts.md)
- [RFC 0012: Python interop boundaries](0012-python-interop-boundaries.md)
- [RFC 0013: Realization and run-provenance wire](0013-realization-and-run-provenance-wire.md)
- [RFC 0014: Production time backend contracts](0014-production-time-backend-contracts.md)
- [RFC 0015: Bounded Gmsh simplex import](0015-bounded-gmsh-simplex-import.md)
- [RFC 0016: Studio as an accessible canonical projection](0016-studio-accessible-projection.md)
- [RFC 0017: Replicated linear execution and fixed-order reductions](0017-replicated-linear-execution.md)
- [RFC 0018: Ordered assembly execution](0018-ordered-assembly-execution.md)
- [RFC 0019: Device execution contracts](0019-device-execution-contracts.md)
- [RFC 0020: Device-neutral local-action kernel boundary](0020-local-action-kernel-boundary.md)
- [RFC 0021: Component hierarchy and deterministic instantiation](0021-component-hierarchy-and-instantiation.md)
- [RFC 0022: Exact package identity and resolution](0022-exact-package-identity-and-resolution.md)
- [RFC 0023: Finalized spatial linear handoff](0023-finalized-spatial-linear-handoff.md)
- [RFC 0024: Scalar conserving connection semantics](0024-scalar-conserving-connection-semantics.md)
- [RFC 0025: Discrete field and external-import provenance](0025-discrete-field-and-import-provenance.md)
- [RFC 0026: Distributed spatial layout and replicated finish](0026-distributed-spatial-layout-and-replication.md)
- [RFC 0027: Capability-rooted package directory admission](0027-capability-rooted-package-directory-admission.md)
- [RFC 0028: Retained local package-store replay](0028-retained-local-package-store-replay.md)
- [RFC 0029: Atomic package-store installation](0029-atomic-package-store-installation.md)
- [RFC 0030: Symbolic package-definition validation](0030-symbolic-package-definition-validation.md)
- [RFC 0031: Joint scalar-physical and exact-periodic reference execution](0031-joint-physical-periodic-reference-execution.md)
- [RFC 0032: Typed package execution lineage](0032-typed-package-execution-lineage.md)
- [RFC 0033: Hierarchical conserving connection sets](0033-hierarchical-conserving-connection-sets.md)
- [RFC 0034: Occurrence-bound spatial supports](0034-occurrence-bound-spatial-supports.md)
- [RFC 0035: Field-valued physical boundary interfaces](0035-field-valued-boundary-interfaces.md)
- [RFC 0036: Physical exposure projection artifacts](0036-physical-exposure-projection-artifacts.md)
- [RFC 0037: Version-neutral typed Model artifact reference](0037-version-neutral-model-artifact-reference.md)
- [RFC 0038: Canonical tensor structure operators and explicit wire v4](0038-canonical-tensor-structure-operators.md)
- [RFC 0039: Canonical two-dimensional isotropic elasticity realization](0039-canonical-isotropic-elasticity-2d.md)
- [RFC 0040: Occurrence-bound Field slots](0040-occurrence-bound-field-slots.md)
- [RFC 0041: Complete-exterior Port families](0041-complete-exterior-port-families.md)
- [RFC 0042: Conforming elasticity interface realization](0042-conforming-elasticity-interface-realization.md)
- [RFC 0043: Simplicial MINI Stokes numerical realization](0043-simplicial-mini-stokes-realization.md)
- [RFC 0044: Packaged steady incompressible Newtonian 2D law](0044-packaged-steady-incompressible-newtonian-2d.md)
- [RFC 0045: Field-wise mixed Realization and coherent-SI congruence](0045-fieldwise-mixed-realization-and-si-congruence.md)
- [RFC 0046: Power-conjugate mechanical boundaries and Port-closed Stokes](0046-power-conjugate-mechanical-boundaries.md)
- [RFC 0047: Mixed Stokes static pressure and boundary-determined pressure](0047-mixed-stokes-static-pressure.md)
- [RFC 0048: First-order dynamic linear-solid semantics](0048-dynamic-linear-solid-semantics.md)
- [RFC 0049: Geometry identity and mesh correspondence](0049-geometry-identity-and-mesh-correspondence.md)
- [RFC 0050: Fixed-reference monolithic fluid--structure interaction](0050-fixed-reference-monolithic-fsi.md)
- [RFC 0051: Durable spatial state and trajectory artifacts](0051-durable-spatial-state-and-trajectory.md)
- [RFC 0052: CAD realization and semantic selection](0052-cad-semantic-selection.md)
- [RFC 0053: Private physics-neutral discrete block system](0053-discrete-block-system.md)
- [RFC 0054: Curated facade and one control plane](0054-curated-facade-and-control-plane.md)
- [RFC 0055: Identity-preserving Component Parameter terms](0055-component-parameter-terms.md)
- [RFC 0056: Proof-carrying pure calculus and semantic support maps](0056-pure-calculus-and-support-maps.md)
- [RFC 0057: Canonical pure-operator definitions](0057-canonical-pure-operator-definitions.md)
- [RFC 0058: Portable Realization and bound execution graphs](0058-portable-realization-and-execution-graphs.md)
- [RFC 0059: One production-workspace MSRV contract](0059-production-msrv-contract.md)
- [RFC 0060: Distributed spatial ownership and owner-routed assembly](0060-distributed-spatial-ownership-and-assembly.md)
- [RFC 0061: MPI fixed-mesh fluid--structure interaction](0061-mpi-fixed-mesh-fsi.md)
- [RFC 0062: Single-device fixed-mesh fluid--structure interaction](0062-cuda-fixed-mesh-fsi.md)
- [RFC 0063: Host-staged MPI plus CUDA fixed-mesh fluid--structure interaction](0063-mpi-cuda-fixed-mesh-fsi.md)
- [RFC 0064: Fixed-topology ALE fluid--structure interaction](0064-fixed-topology-ale-fsi.md)
- [RFC 0065: Remeshing correspondence and conservative FSI transfer](0065-remeshing-correspondence-and-transfer.md)
- [RFC 0066: Remeshing trajectory XDMF/HDF5 export](0066-remeshing-trajectory-xdmf-hdf5-export.md)
- [RFC 0067: Derived ML Dataset over durable spatial trajectories](0067-derived-ml-dataset.md)
- [RFC 0068: Optional implementation-agent attestations](0068-optional-implementation-agent-attestations.md)
- [RFC 0069: Conservative cell-centered scalar transport](0069-conservative-cell-centered-transport.md)
- [RFC 0070: Dimension-parametric tetrahedral ALE fluid--structure interaction](0070-dimension-parametric-tetrahedral-ale-fsi.md)
- [RFC 0071: Spatial-periodic boundary connection](0071-spatial-periodic-boundary-connection.md)
- [RFC 0072: Collocated incompressible finite-volume realization](0072-collocated-incompressible-finite-volume.md)
- [RFC 0073: Structural semantic fingerprint](0073-structural-semantic-fingerprint.md)
- [RFC 0074: Eqiora public alpha identity](0074-public-alpha-identity.md)
- [RFC 0075: FEM form compiler, Cartesian Q1 Poisson slice](0075-fem-form-compiler-poisson-q1.md)
- [RFC 0076: Evidence-first Studio interaction](0076-evidence-first-studio-interaction.md)
- [RFC 0077: Exact topology-preserving Cartesian Domain edit](0077-exact-cartesian-domain-edit.md)
- [RFC 0078: Direct Parameter-driven Cartesian coordinates](0078-direct-parameter-driven-cartesian-coordinates.md)
- [RFC 0079: Authored planar geometry artifact](0079-authored-planar-geometry-artifact.md)
- [RFC 0080: Geometry-backed semantic admission](0080-geometry-backed-semantic-admission.md)
- [RFC 0081: Exact circular-hole planar geometry](0081-exact-circular-hole-geometry.md)
- [RFC 0082: Source-bound chordal circular-hole reference mesh](0082-source-bound-chordal-circular-hole-mesh.md)
- [RFC 0083: One current Model artifact epoch before 1.0](0083-current-model-artifact-epoch.md)
- [RFC 0084: Contract-wave capability development](0084-contract-wave-capability-development.md)
- [RFC 0085: Standalone prescribed dynamic-solid artifacts](0085-standalone-prescribed-dynamic-solid-artifacts.md)
- [RFC 0000: Private numerics--differentiation composition](0000-private-numerics-differentiation-composition.md)
