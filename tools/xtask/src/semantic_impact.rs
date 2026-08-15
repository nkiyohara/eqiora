#[path = "semantic_impact/cargo.rs"]
mod cargo;
#[path = "semantic_impact/change.rs"]
mod change;
#[path = "semantic_impact/model.rs"]
mod model;
#[path = "semantic_impact/repository.rs"]
mod repository;

pub(crate) use model::{
    AnalysisCaps, AnalysisFailure, AnalysisHead, CargoAuthorityAnalysis, CargoAuthorityRecord,
    CargoAuthorityRequest, CargoContentRecord, CargoDependencyKind, CargoGraphAuthority,
    CargoTargetKind, CaseId, CfgProfile, CommitRevision, Completeness, CoverageAuthority,
    CoverageCertificate, ExactAtom, ExactBytes, ExactOverlay, ExactQueryDomain, ExactRepoPath,
    FullGitOid, GitMode, NonEmptySortedSet, NonEmptySortedVec, OverlayEntry, OverlayStatus,
    RevisionIdentity, RevisionPoint, RevisionSide, Sha256, SortedSet,
};
pub(crate) use repository::analyze_cargo_authority;
