//! Shared private vocabulary for proof-carrying FEM formulations.

use eqiora_core::RawId;
use eqiora_schema::kernel::ExprId;

pub(super) const TEST_PAIRING: &str = "fem.derive.v1.test-pairing";
pub(super) const DIVERGENCE_BY_PARTS: &str = "fem.derive.v1.divergence-by-parts";
pub(super) const HOMOGENEOUS_ESSENTIAL_DISCHARGE: &str =
    "fem.derive.v1.boundary-discharge.essential-homogeneous";
pub(super) const SOURCE_PAIRING: &str = "fem.derive.v1.source-pairing";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MatrixSlot {
    Test,
    Trial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WeakTermSlot {
    TestPairing { test: MatrixSlot },
    Bilinear { test: MatrixSlot, trial: MatrixSlot },
    Boundary { test: MatrixSlot },
    Linear { test: MatrixSlot },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WeakSign {
    Positive,
    Negative,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormulationKind {
    PrimalGalerkin,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BoundaryTreatment {
    CompleteHomogeneousEssential,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FormulationRule {
    TestPairing,
    DivergenceByParts,
    HomogeneousEssentialDischarge,
    SourcePairing,
}

impl FormulationRule {
    const fn id(self) -> &'static str {
        match self {
            Self::TestPairing => TEST_PAIRING,
            Self::DivergenceByParts => DIVERGENCE_BY_PARTS,
            Self::HomogeneousEssentialDischarge => HOMOGENEOUS_ESSENTIAL_DISCHARGE,
            Self::SourcePairing => SOURCE_PAIRING,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LawIdentity {
    pub(super) domain: RawId,
    pub(super) unknown: RawId,
    pub(super) relations: Vec<RawId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EffectiveFormulation {
    pub(super) kind: FormulationKind,
    pub(super) trial: RawId,
    pub(super) test: RawId,
    pub(super) boundary_treatment: BoundaryTreatment,
    pub(super) rules: [FormulationRule; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CertificateEntry {
    pub(super) rule_id: &'static str,
    pub(super) relation: RawId,
    pub(super) source_node: ExprId,
    pub(super) slot: WeakTermSlot,
    pub(super) sign: WeakSign,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct BoundarySource {
    pub(super) relation: RawId,
    pub(super) trace_node: ExprId,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PrimalGalerkinSource<'a> {
    pub(super) domain: RawId,
    pub(super) unknown: RawId,
    pub(super) volume_relation: RawId,
    pub(super) root: ExprId,
    pub(super) divergence: ExprId,
    pub(super) source: ExprId,
    pub(super) boundaries: &'a [BoundarySource],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PrimalGalerkinCorrespondence {
    pub(super) law: LawIdentity,
    pub(super) formulation: EffectiveFormulation,
    pub(super) entries: Vec<CertificateEntry>,
}

impl PrimalGalerkinCorrespondence {
    pub(super) fn derive(source: PrimalGalerkinSource<'_>) -> Self {
        let rules = [
            FormulationRule::TestPairing,
            FormulationRule::DivergenceByParts,
            FormulationRule::HomogeneousEssentialDischarge,
            FormulationRule::SourcePairing,
        ];
        let mut relations = Vec::with_capacity(source.boundaries.len() + 1);
        relations.push(source.volume_relation);
        relations.extend(source.boundaries.iter().map(|boundary| boundary.relation));

        let mut entries = Vec::with_capacity(source.boundaries.len() + 3);
        entries.push(CertificateEntry {
            rule_id: rules[0].id(),
            relation: source.volume_relation,
            source_node: source.root,
            slot: WeakTermSlot::TestPairing {
                test: MatrixSlot::Test,
            },
            sign: WeakSign::Positive,
        });
        entries.push(CertificateEntry {
            rule_id: rules[1].id(),
            relation: source.volume_relation,
            source_node: source.divergence,
            slot: WeakTermSlot::Bilinear {
                test: MatrixSlot::Test,
                trial: MatrixSlot::Trial,
            },
            sign: WeakSign::Positive,
        });
        entries.extend(source.boundaries.iter().map(|boundary| CertificateEntry {
            rule_id: rules[2].id(),
            relation: boundary.relation,
            source_node: boundary.trace_node,
            slot: WeakTermSlot::Boundary {
                test: MatrixSlot::Test,
            },
            sign: WeakSign::Negative,
        }));
        entries.push(CertificateEntry {
            rule_id: rules[3].id(),
            relation: source.volume_relation,
            source_node: source.source,
            slot: WeakTermSlot::Linear {
                test: MatrixSlot::Test,
            },
            sign: WeakSign::Positive,
        });

        Self {
            law: LawIdentity {
                domain: source.domain,
                unknown: source.unknown,
                relations,
            },
            formulation: EffectiveFormulation {
                kind: FormulationKind::PrimalGalerkin,
                trial: source.unknown,
                test: source.unknown,
                boundary_treatment: BoundaryTreatment::CompleteHomogeneousEssential,
                rules,
            },
            entries,
        }
    }

    pub(super) fn replay(&self, source: PrimalGalerkinSource<'_>) -> Result<(), &'static str> {
        if self != &Self::derive(source) {
            return Err("Law identity, effective Formulation, or correspondence steps are stale");
        }
        Ok(())
    }
}
