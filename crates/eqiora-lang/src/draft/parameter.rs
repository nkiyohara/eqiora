//! Complete uniform Parameter values and their optional Cartesian frame reference.

use super::*;

impl DraftParameter {
    /// Declare one complete Parameter value in coherent SI units.
    #[must_use]
    pub fn new(name: impl Into<String>, value: ValueLiteral) -> Self {
        Self {
            symbol: DraftSymbol::new(),
            name: name.into(),
            value,
            frame: None,
        }
    }

    /// Interpret spatial components in this exact draft-local Cartesian frame.
    /// The Parameter remains uniform and does not acquire spatial support.
    #[must_use]
    pub fn with_frame(mut self, frame: &DraftSpatialDomain) -> Self {
        self.frame = Some(frame.clone());
        self
    }

    pub(super) fn frame_name(&self, range: TextRange) -> Option<crate::NamePath> {
        self.frame
            .as_ref()
            .map(|frame| crate::NamePath::single(frame.name().to_owned(), range))
    }

    /// Declaration name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Static SI dimension.
    #[must_use]
    pub const fn dimension(&self) -> DimExponents {
        self.value.value_type().dimension()
    }

    /// Complete declared mathematical type.
    #[must_use]
    pub const fn value_type(&self) -> &ValueType {
        self.value.value_type()
    }

    /// Complete value in coherent SI units, without scalar projection.
    #[must_use]
    pub const fn value(&self) -> &ValueLiteral {
        &self.value
    }

    /// Use this Parameter as a typed expression.
    #[must_use]
    pub fn expression(&self) -> DraftExpression {
        DraftExpression::reference(
            self.symbol.clone(),
            self.name.clone(),
            DraftSymbolKind::Parameter,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{ScalarDomain, ValueFrame, ValueShape};

    fn coefficient() -> ValueLiteral {
        ValueLiteral::new(
            ValueType::shaped(
                ScalarDomain::Real,
                DimExponents::DIMENSIONLESS,
                ValueShape::new([2, 2]).unwrap(),
                ValueFrame::SpatialCartesian,
            )
            .unwrap(),
            [(2.0, 0.0), (3.0, 0.0), (5.0, 0.0), (7.0, 0.0)],
        )
        .unwrap()
    }

    #[test]
    fn native_frame_is_exact_owned_domain_without_parameter_support() {
        let body = DraftSpatialDomain::cartesian_box("body", [(0.0, 1.0); 2]);
        let boundary =
            DraftSpatialDomain::boundary("wall", &body, 0, crate::BoundarySideSyntax::Lower);
        for frame in [&body, &boundary] {
            let parameter = DraftParameter::new("coefficient", coefficient()).with_frame(frame);
            let draft = Module::new(
                "M",
                [
                    body.clone().into(),
                    boundary.clone().into(),
                    parameter.into(),
                ],
            )
            .unwrap();
            let rendered = crate::format(draft.document());
            assert!(rendered.contains(&format!(
                "tensor_value(frame = {}, components = [[2, 3], [5, 7]])",
                frame.name()
            )));
            assert!(
                crate::parse("native.eqi", &rendered)
                    .into_document()
                    .is_ok()
            );
        }
        let foreign = DraftSpatialDomain::cartesian_box("body", [(0.0, 1.0); 2]);
        let error = Module::new(
            "M",
            [
                body.clone().into(),
                DraftParameter::new("a", coefficient())
                    .with_frame(&foreign)
                    .into(),
            ],
        )
        .unwrap_err();
        assert!(
            error
                .iter()
                .any(|e| e.message().contains("foreign or omitted"))
        );
        assert!(
            Module::new(
                "M",
                [body.into(), DraftParameter::new("a", coefficient()).into()]
            )
            .is_err()
        );
        let one_d = DraftSpatialDomain::cartesian_box("line", [(0.0, 1.0)]);
        assert!(
            Module::new(
                "M",
                [
                    one_d.clone().into(),
                    DraftParameter::new("a", coefficient())
                        .with_frame(&one_d)
                        .into()
                ]
            )
            .is_err()
        );
    }
}
