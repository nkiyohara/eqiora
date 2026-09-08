//! Nominal discrete predicates use the same typed demand evaluator as numeric branches.
use super::*;
use eqiora_core::{Id, ValueLiteral, ValueType};
use eqiora_schema::kernel::{ComparisonOp, ExprDagBuilder};

#[test]
fn enum_predicates_select_numeric_values_without_reading_inactive_symbols() {
    let ty = ValueType::enumeration(Id::new(), 2).unwrap();
    let mode = SymbolRef::Parameter(Id::new());
    let inactive = SymbolRef::Parameter(Id::new());
    let mut builder = ExprDagBuilder::new();
    let input = builder.symbol(mode).unwrap();
    let member = builder
        .constant(ValueLiteral::enum_value(ty.clone(), 1).unwrap())
        .unwrap();
    let equal = builder.compare(ComparisonOp::Equal, input, member).unwrap();
    let unequal = builder
        .compare(ComparisonOp::NotEqual, input, member)
        .unwrap();
    let numeric = builder
        .constant(eqiora_core::DynQuantity::new(
            7.0,
            eqiora_core::DimExponents::DIMENSIONLESS,
        ))
        .unwrap();
    let missing = builder.symbol(inactive).unwrap();
    let selected = builder.select(equal, numeric, missing).unwrap();
    let dag = builder.finish([equal, unequal, selected]).unwrap();
    let ir = ScalarOperatorIr::lower(&dag).unwrap();
    let values = ir
        .evaluate_typed(&[equal, unequal, selected], &mut |symbol| {
            assert_eq!(symbol, mode, "inactive branch must not resolve its symbol");
            Some(ValueLiteral::enum_value(ty.clone(), 1).unwrap())
        })
        .unwrap();
    assert_eq!(values[0].as_bool(), Some(true));
    assert_eq!(values[1].as_bool(), Some(false));
    assert_eq!(values[2].real_scalar_value().unwrap().value(), 7.0);
    assert!(
        ir.evaluate_typed(&[equal], &mut |_| Some(
            ValueLiteral::enum_value(ValueType::enumeration(Id::new(), 2).unwrap(), 1).unwrap()
        ))
        .is_err()
    );
    let values = ir
        .evaluate_typed(&[equal, unequal], &mut |_| {
            Some(ValueLiteral::enum_value(ty.clone(), 0).unwrap())
        })
        .unwrap();
    assert_eq!(values[0].as_bool(), Some(false));
    assert_eq!(values[1].as_bool(), Some(true));
}

#[test]
fn enums_have_no_numeric_execution_or_differentiation_profile() {
    let ty = ValueType::enumeration(Id::new(), 2).unwrap();
    let value = ValueLiteral::enum_value(ty, 0).unwrap();
    let mut builder = ExprDagBuilder::new();
    let input = builder.symbol(SymbolRef::Parameter(Id::new())).unwrap();
    let member = builder.constant(value.clone()).unwrap();
    let ordered = builder.compare(ComparisonOp::Less, input, member).unwrap();
    let sum = builder.add(input, member).unwrap();
    let dag = builder.finish([input, ordered, sum]).unwrap();
    let ir = ScalarOperatorIr::lower(&dag).unwrap();
    assert_eq!(
        ir.evaluate_typed(&[input], &mut |_| Some(value.clone()))
            .unwrap()[0],
        value
    );
    for root in [ordered, sum] {
        assert!(
            ir.evaluate_typed(&[root], &mut |_| Some(value.clone()))
                .is_err()
        );
    }
    assert!(ir.evaluate(&[0.0]).is_err());
    for role in [
        DifferentiationRole::Unknown,
        DifferentiationRole::Parameter,
        DifferentiationRole::Frozen,
    ] {
        assert!(
            ir.linearize_typed(std::slice::from_ref(&value), &[role])
                .unwrap_err()
                .message()
                .contains("real scalar")
        );
    }
}
