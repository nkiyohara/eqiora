//! Separate canonical identity for authored formulations.

use super::*;

const MAGIC: &[u8; 8] = b"EQIORAFM";

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AuthoredFormSourceIdentity([u8; 32]);

impl AuthoredFormSourceIdentity {
    pub(crate) fn from_component(component: &ComponentDecl) -> Result<Self, Diagnostic> {
        let limits = LocalSourceIdentityLimits::default();
        let mut budget = Budget::new(limits);
        budget.account_members(component.formulations().len(), "Formulation")?;
        let declarations = component.formulations().collect::<Vec<_>>();
        let formulations = encode_sorted_records(
            &declarations,
            &mut budget,
            |(relation, left, right, _), budget| {
                let mut encoder = Encoder::new(budget.limits.max_canonical_bytes);
                encoder.field(1, |encoder| encoder.u16(1))?;
                encoder.field(2, |encoder| encode_name(encoder, relation, budget))?;
                encoder.field(3, |encoder| encode_expression(encoder, left, budget, 1))?;
                encoder.field(4, |encoder| encode_expression(encoder, right, budget, 1))?;
                encoder.finish()
            },
        )?;
        let mut encoder = Encoder::new(limits.max_canonical_bytes);
        encoder.raw(MAGIC)?;
        encoder.u16(CANONICAL_VERSION)?;
        encoder.field(1, |encoder| {
            encode_name(encoder, component.name(), &mut budget)
        })?;
        encoder.field(2, |encoder| encoder.records(&formulations))?;
        Ok(Self(Sha256::digest(encoder.finish()?).into()))
    }
}

impl fmt::Debug for AuthoredFormSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "AuthoredFormSourceIdentity({self})")
    }
}

impl fmt::Display for AuthoredFormSourceIdentity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use eqiora_lang::{format, parse};

    use super::*;

    fn document(source: &str) -> Document {
        parse("fixture.eqi", source).into_document().unwrap()
    }

    #[test]
    fn identity_is_separate_from_model_allocation_identity() {
        let without = "component D { relation balance continuous { 1 = 0; } }";
        let first = "component D { relation balance continuous { 1 = 0; } form primal for balance { integrate(region, dot(grad(test(u)), grad(u))) = integrate(region, test(u) * f); } }";
        let changed = "component D { relation balance continuous { 1 = 0; } form primal for balance { integrate(region, dot(grad(test(u)), k * grad(u))) = integrate(region, test(u) * f); } }";
        let model_identity =
            |source| LocalSourceIdentity::from_document(&document(source)).unwrap();
        let form_identity = |source| {
            AuthoredFormSourceIdentity::from_component(&document(source).components()[0]).unwrap()
        };

        assert_eq!(model_identity(without), model_identity(first));
        assert_eq!(model_identity(first), model_identity(changed));
        assert_eq!(
            model_identity(first),
            model_identity(&format(&document(first)))
        );
        assert_ne!(form_identity(without), form_identity(first));
        assert_ne!(form_identity(first), form_identity(changed));
        assert_eq!(
            form_identity(first),
            form_identity(&format(&document(first)))
        );
    }
}
