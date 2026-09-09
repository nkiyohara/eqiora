use super::Writer;
use eqiora_core::{Diagnostic, ScalarDomain, ValueFrame, ValueType};
use eqiora_lang::NotationProfile;

impl Writer {
    pub(super) fn value_type(&mut self, value: &ValueType) -> Result<(), Diagnostic> {
        let ml = self.profile == NotationProfile::MathMl;
        let latex = self.profile == NotationProfile::Latex;
        let domain = match value.scalar_domain() {
            ScalarDomain::Real => "real",
            ScalarDomain::Complex => "complex",
            ScalarDomain::Integer => "integer",
            ScalarDomain::Boolean => "Boolean",
            ScalarDomain::Enum => "enum",
        };
        // All type prose comes from checked scalar-domain/shape/frame nodes.
        // Rational dimension pairs remain exact, never decimalized or guessed.
        if ml {
            self.push("<mtext>")?;
        } else if latex {
            self.push("\\text{")?;
        }
        self.text(domain)?;
        for (name, id) in [
            (
                " enum declaration ",
                value.enum_definition().map(|id| id.to_string()),
            ),
            (" index set ", value.index_set().map(|id| id.to_string())),
            (
                if value.is_count() {
                    " count basis "
                } else {
                    " coordinate basis "
                },
                value.finite_space().map(|id| id.to_string()),
            ),
        ] {
            if let Some(id) = id {
                self.text(name)?;
                self.text(&id)?;
            }
        }
        for (axis, extent) in value.shape().extents().iter().enumerate() {
            self.text(if axis < value.array_rank() {
                " channel axis "
            } else if value.frame() == ValueFrame::SpatialCartesian {
                " spatial axis "
            } else {
                " component axis "
            })?;
            self.text(&extent.to_string())?;
        }
        if let Some(count) = value.enum_member_count() {
            self.text(" enum members ")?;
            self.text(&count.to_string())?;
        }
        if let Some(extent) = value.index_extent() {
            self.text(" index extent ")?;
            self.text(&extent.to_string())?;
        }
        self.text(match value.frame() {
            ValueFrame::Invariant => " invariant frame",
            ValueFrame::SpatialCartesian => " Cartesian frame",
        })?;
        self.text("; dimensions ")?;
        let mut has_unit = false;
        for (unit, (numerator, denominator)) in ["kg", "m", "s", "A", "K", "mol", "cd"]
            .into_iter()
            .zip(value.dimension().exponents())
        {
            if numerator == 0 {
                continue;
            }
            if has_unit {
                self.text(" times ")?;
            }
            has_unit = true;
            self.text(unit)?;
            if numerator != 1 || denominator != 1 {
                self.text(" to power ")?;
                self.text(&numerator.to_string())?;
                if denominator != 1 {
                    self.text(" over ")?;
                    self.text(&denominator.to_string())?;
                }
            }
        }
        if !has_unit {
            self.text("dimensionless")?;
        }
        if ml {
            self.push("</mtext>")?;
        } else if latex {
            self.push("}")?;
        }
        Ok(())
    }
}
