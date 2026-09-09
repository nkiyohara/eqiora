//! Identity allocation for staged lowering.
use super::*;

/// Identity source for one completely staged lowering.
///
/// Supplies collision-checked hierarchical identities or fresh flat identities.
pub(crate) trait LoweringIdentities {
    fn model(&mut self, name: &str) -> OntologyId<Model>;

    fn domain(&mut self, name: &str) -> Id<kinds::Domain>;

    fn representation(&mut self, name: &str) -> Id<kinds::Representation>;

    fn field(&mut self, name: &str) -> Id<kinds::Field>;

    fn parameter(&mut self, name: &str) -> Id<kinds::Parameter>;

    fn port(&mut self, name: &str) -> Id<kinds::Port>;

    fn clock(&mut self, name: &str) -> Id<kinds::ClockDomain>;

    fn observable(&mut self, name: &str) -> Id<kinds::Observable>;

    fn activation(&mut self, name: &str) -> Id<kinds::Activation>;

    fn relation(&mut self, name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>);

    fn connection(&mut self) -> Id<kinds::Connection>;
}

#[cfg(test)]
pub(super) struct FreshLoweringIdentities;

#[cfg(test)]
impl LoweringIdentities for FreshLoweringIdentities {
    fn model(&mut self, _name: &str) -> OntologyId<Model> {
        OntologyId::new()
    }

    fn domain(&mut self, _name: &str) -> Id<kinds::Domain> {
        Id::new()
    }

    fn representation(&mut self, _name: &str) -> Id<kinds::Representation> {
        Id::new()
    }

    fn field(&mut self, _name: &str) -> Id<kinds::Field> {
        Id::new()
    }

    fn parameter(&mut self, _name: &str) -> Id<kinds::Parameter> {
        Id::new()
    }

    fn port(&mut self, _name: &str) -> Id<kinds::Port> {
        Id::new()
    }

    fn clock(&mut self, _name: &str) -> Id<kinds::ClockDomain> {
        Id::new()
    }

    fn observable(&mut self, _name: &str) -> Id<kinds::Observable> {
        Id::new()
    }

    fn activation(&mut self, _name: &str) -> Id<kinds::Activation> {
        Id::new()
    }

    fn relation(&mut self, _name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>) {
        (Id::new(), Id::new())
    }

    fn connection(&mut self) -> Id<kinds::Connection> {
        Id::new()
    }
}
