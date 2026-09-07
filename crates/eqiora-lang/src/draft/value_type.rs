use super::*;
use crate::{SourceAstFactory, ValueTypeSyntax, ValueTypeSyntaxKind};

pub(super) fn validate(value: &ValueType) -> Result<(), String> {
    ValueTypeSyntax::from_checked(value)
        .map(|_| ())
        .map_err(|error| error.to_string())
}

impl ValueTypeSyntax {
    /// Project a checked mathematical type into the bounded source grammar.
    ///
    /// # Errors
    /// Rejects types exceeding source constructor or component-count limits.
    pub fn from_checked(value: &ValueType) -> Result<Self, crate::AstConstructionError> {
        // Bound projection work before constructing the source AST. The shared
        // factory then applies the exact source constructor and element-count rules.
        if value.array_rank() >= 256 || value.shape().rank() - value.array_rank() > 4 {
            return Err(crate::AstConstructionError::new(
                "mathematical type exceeds source nesting or spatial-rank limits",
            ));
        }
        let syntax = project(
            value,
            &GraphPath::new(["value-type"]),
            &mut RangeAllocator::default(),
            &mut HashMap::new(),
        );
        SourceAstFactory::value_type(syntax.kind, syntax.range)
    }
}

pub(super) fn project(
    value: &ValueType,
    path: &GraphPath,
    ranges: &mut RangeAllocator,
    paths: &mut HashMap<TextRange, GraphPath>,
) -> ValueTypeSyntax {
    let mut syntax = ValueTypeSyntax {
        kind: ValueTypeSyntaxKind::Scalar {
            domain: value.scalar_domain(),
            dimension: dimension_expression(value.dimension(), path, ranges, paths),
        },
        range: ranges.allocate(path, paths),
    };
    let (arrays, spatial) = value.shape().extents().split_at(value.array_rank());
    if !spatial.is_empty() {
        let kind = if spatial.len() == 1 {
            ValueTypeSyntaxKind::Vector {
                scalar: Box::new(syntax),
                extent: spatial[0].get(),
            }
        } else {
            ValueTypeSyntaxKind::Tensor {
                scalar: Box::new(syntax),
                extents: spatial.iter().map(|n| n.get()).collect(),
            }
        };
        syntax = ValueTypeSyntax {
            kind,
            range: ranges.allocate(path, paths),
        };
    }
    for extent in arrays.iter().rev() {
        syntax = ValueTypeSyntax {
            kind: ValueTypeSyntaxKind::Array {
                element: Box::new(syntax),
                extent: extent.get(),
            },
            range: ranges.allocate(path, paths),
        };
    }
    syntax
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Item;
    use eqiora_core::{ScalarDomain, ValueFrame, ValueShape};

    #[test]
    fn native_parameters_preserve_declared_types_in_source_projection() {
        let scalar = ValueType::scalar(ScalarDomain::Complex, DimExponents::DIMENSIONLESS);
        for (value_type, expected) in [
            (scalar.clone(), "complex<1>"),
            (scalar.array(3).unwrap(), "array<complex<1>, 3>"),
        ] {
            let parameter = DraftParameter::new(
                "coefficient",
                eqiora_core::ValueLiteral::from_real(value_type.clone(), 0.0).unwrap(),
            );
            assert_eq!(parameter.value_type(), &value_type);
            let draft = ModelDraft::new("M", [parameter.into()]).unwrap();
            let native = draft.native_ast();
            let Item::Parameter(parameter) = &native.model().items()[0] else {
                panic!("Parameter");
            };
            assert_eq!(parameter.value_type().to_source(), expected);
            assert_eq!(
                native
                    .graph_path(parameter.value_type().range())
                    .unwrap()
                    .to_string(),
                "M.coefficient"
            );
            let document =
                SourceAstFactory::document(vec![], vec![], vec![native.model().clone()]).unwrap();
            let source = crate::format(&document);
            let parsed = crate::parse("native.eqi", &source).into_document().unwrap();
            assert_eq!(crate::format(&parsed), source);
        }
        let oversized = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS)
            .array(65_537)
            .unwrap();
        let parameter = DraftParameter::new(
            "large",
            eqiora_core::ValueLiteral::from_real(oversized, 0.0).unwrap(),
        );
        assert!(ModelDraft::new("M", [parameter.into()]).is_err());
    }

    #[test]
    fn native_fields_keep_nested_types_and_absent_initial_values() {
        let value = ValueType::shaped(
            ScalarDomain::Complex,
            DimExponents::DIMENSIONLESS,
            ValueShape::new([2]).unwrap(),
            ValueFrame::SpatialCartesian,
        )
        .unwrap()
        .array(3)
        .unwrap();
        let field = DraftField::new("channels", value.clone(), crate::FieldRoleSyntax::Variable);
        assert_eq!(field.value_type(), &value);
        assert_eq!(field.role(), crate::FieldRoleSyntax::Variable);
        let draft = ModelDraft::new("M", [field.into()]).unwrap();
        let native = draft.native_ast();
        let document =
            SourceAstFactory::document(vec![], vec![], vec![native.model().clone()]).unwrap();
        let source = crate::format(&document);
        assert!(source.contains("variable channels: array<vector<complex<1>, 2>, 3>;"));
        let parsed = crate::parse("native.eqi", &source).into_document().unwrap();
        assert_eq!(crate::format(&parsed), source);
        let Item::Field(field) = &native.model().items()[0] else {
            panic!("Field");
        };
        assert_eq!(
            native
                .graph_path(field.value_type().range())
                .unwrap()
                .to_string(),
            "M.channels"
        );
    }

    #[test]
    fn native_fields_obey_source_type_limits() {
        let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS);
        let oversized = DraftField::new(
            "large",
            scalar.array(65_537).unwrap(),
            crate::FieldRoleSyntax::Variable,
        );
        assert!(ModelDraft::new("M", [oversized.into()]).is_err());
    }
}
