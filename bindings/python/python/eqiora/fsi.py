"""Fixed-mesh monolithic FSI intent, Plan, and typed evidence."""

from ._eqiora import (
    FixedMeshMonolithic,
    FixedMeshMonolithicEvidence,
    FixedMeshMonolithicPlan,
    FixedMeshMonolithicStateEvidence,
    fixed_mesh_monolithic_evidence,
    resolve_fixed_mesh_monolithic as resolve,
)

__all__ = [
    "FixedMeshMonolithic",
    "FixedMeshMonolithicEvidence",
    "FixedMeshMonolithicPlan",
    "FixedMeshMonolithicStateEvidence",
    "fixed_mesh_monolithic_evidence",
    "resolve",
]
