use super::*;
use eqiora_core::{DimExponents, ScalarDomain};
use eqiora_schema::kernel::{ComparisonOp, UnaryMathFunction};
use serde_json::json;

fn definition() -> PureOperatorDefinition {
    let area = DimExponents::from_integers([0, 2, 0, 0, 0, 0, 0]).unwrap();
    let length = DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).unwrap();
    let real = PureValueClass::invariant_scalar()
        .with_scalar_domain(ScalarDomain::Real)
        .unwrap();
    let mut b =
        CalculusBuilder::new([real.with_dimension(area)], real.with_dimension(length)).unwrap();
    let x = b
        .push(CalculusNode::FormalComponent {
            formal: 0,
            axes: Box::new([]),
        })
        .unwrap();
    let zero = b
        .push(CalculusNode::Rational {
            value: ExactRational::new(0, 1).unwrap(),
            dimension: area,
        })
        .unwrap();
    let nonnegative = b
        .push(CalculusNode::Compare(ComparisonOp::GreaterEqual, x, zero))
        .unwrap();
    let no = b.push(CalculusNode::Boolean(false)).unwrap();
    let yes = b.push(CalculusNode::Not(no)).unwrap();
    let both = b.push(CalculusNode::And(nonnegative, yes)).unwrap();
    let either = b.push(CalculusNode::Or(both, no)).unwrap();
    let sqrt = b
        .push(CalculusNode::UnaryMath(UnaryMathFunction::Sqrt, x))
        .unwrap();
    let one = b
        .push(CalculusNode::Rational {
            value: ExactRational::new(1, 1).unwrap(),
            dimension: length,
        })
        .unwrap();
    let selected = b
        .push(CalculusNode::Select {
            condition: both,
            then_value: sqrt,
            else_value: one,
        })
        .unwrap();
    let required = b
        .push(CalculusNode::Require {
            condition: either,
            value: selected,
        })
        .unwrap();
    b.finish(required).unwrap()
}

#[test]
fn mixed_calculus_wire_retains_typed_rationals_and_ordered_guard_roles() {
    let original = definition();
    let wire = WirePureOperatorDefinition::encode(&original);
    let json = serde_json::to_value(&wire).unwrap();
    assert_eq!(
        json["nodes"][1],
        json!({"op":"rational","numerator":0,"denominator":1,"dimension":[[0,1],[2,1],[0,1],[0,1],[0,1],[0,1],[0,1]]})
    );
    assert_eq!(
        json["nodes"][2],
        json!({"op":"compare","comparison":"greater-equal","left":0,"right":1})
    );
    assert_eq!(json["nodes"][3], json!({"op":"boolean","value":false}));
    assert_eq!(
        json["nodes"][7],
        json!({"op":"unary-math","function":"sqrt","value":0})
    );
    assert_eq!(
        json["nodes"][9],
        json!({"op":"select","condition":5,"then_value":7,"else_value":8})
    );
    assert_eq!(
        json["nodes"][10],
        json!({"op":"require","condition":6,"value":9})
    );
    let replay: WirePureOperatorDefinition = serde_json::from_value(json).unwrap();
    assert_eq!(replay.rebuild_and_validate_digest().unwrap(), original);
}

#[test]
fn mixed_calculus_wire_rejects_invalid_types_units_references_and_identity() {
    let original = serde_json::to_value(WirePureOperatorDefinition::encode(&definition())).unwrap();
    for mutation in 0..8 {
        let mut invalid = original.clone();
        match mutation {
            0 => invalid["nodes"][9]["condition"] = json!(99),
            1 => invalid["nodes"][10]["condition"] = json!(7),
            2 => invalid["nodes"][9]["else_value"] = json!(1),
            3 => invalid["nodes"][7]["function"] = json!("sin"),
            4 => invalid["nodes"][1]["dimension"][1] = json!([2, 2]),
            5 => invalid["nodes"][3]["value"] = json!(0),
            6 => invalid["nodes"][2]["comparison"] = json!("less-equal"),
            _ => {
                invalid["nodes"][1]
                    .as_object_mut()
                    .unwrap()
                    .remove("dimension");
            }
        }
        let rejected = match serde_json::from_value::<WirePureOperatorDefinition>(invalid) {
            Err(_) => true,
            Ok(wire) => wire.rebuild_and_validate_digest().is_err(),
        };
        assert!(rejected, "mutation {mutation}");
    }
}
