use eqiora::compiler::compile;
use eqiora::graph::{GraphStore, InMemoryGraphStore, Op, Precondition, Transaction};
use eqiora::ir::{
    DifferentiationRole, LinearizedRelation, RelationCotangent, RelationTangent, ScalarOperatorIr,
};
use eqiora::kernel::{KernelNode, SymbolRef};
use eqiora::sem::{Interpreter, KernelProgram, ReferenceConfig};
use eqiora::{DimExponents, DynQuantity};

const ALIASES: &str = r#"
component Polynomial(
  parameter x: 1,
  parameter y: 1
) {
  let f = z * y;
  let z = x * x;
  variable output: 1;
  relation balance { output = f; }
}
component Wrapper(
  parameter a: 1,
  parameter b: 1
) {
  let shifted = a + 1;
  instance poly: Polynomial(x = shifted, y = b);
}
model M() {
  parameter x: 1 = 2;
  parameter y: 1 = 3;
  instance direct: Polynomial(x = x, y = y);
  instance shifted: Polynomial(x = x + 1, y = y);
  instance wrapped: Wrapper(a = x, b = y);
}
"#;

#[test]
fn static_aliases_preserve_occurrence_values_parameter_edits_and_the_chain_rule() {
    let explicit = ALIASES
        .replace("  let f = z * y;\n  let z = x * x;\n", "")
        .replace("output = f", "output = (x * x) * y")
        .replace("  let shifted = a + 1;\n", "")
        .replace("x = shifted", "x = a + 1");
    for source in [ALIASES, explicit.as_str()] {
        let compiled = compile("static-aliases.eqi", source)
            .unwrap()
            .pop()
            .unwrap();
        let symbols = compiled.symbols().clone();
        let x_id = symbols.get("x").unwrap();
        let y_id = symbols.get("y").unwrap();
        for alias in ["direct.z", "direct.f", "shifted.f", "wrapped.shifted"] {
            assert!(
                symbols.get(alias).is_none(),
                "alias is not an entity: {alias}"
            );
        }
        let (transaction, model, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        for (x, y) in [(2.0, 3.0), (5.0, -2.0)] {
            let mut update = Transaction::new("edit only original physical Parameters");
            update.require(Precondition::RevisionIs(store.snapshot().revision()));
            for (target, value) in [(x_id, x), (y_id, y)] {
                update.push(Op::SetValue {
                    target,
                    value: DynQuantity::new(value, DimExponents::DIMENSIONLESS)
                        .try_into()
                        .unwrap(),
                });
            }
            store.commit(update).unwrap();
            let program = KernelProgram::from_snapshot(&store.snapshot(), model).unwrap();
            assert_eq!(
                program
                    .nodes()
                    .filter(|node| matches!(node, KernelNode::Parameter(_)))
                    .count(),
                2
            );
            assert_eq!(
                program
                    .nodes()
                    .filter(|node| matches!(node, KernelNode::Field(_)))
                    .count(),
                3
            );
            assert_eq!(
                program
                    .nodes()
                    .filter(|node| matches!(node, KernelNode::Relation(_)))
                    .count(),
                3
            );
            let result = Interpreter::new()
                .run(&program, ReferenceConfig::new(0.0, 0.01).unwrap())
                .unwrap();
            for (occurrence, offset) in [("direct", 0.0), ("shifted", 1.0), ("wrapped.poly", 1.0)] {
                let output = symbols.get(&format!("{occurrence}.output")).unwrap();
                let relation = symbols.get(&format!("{occurrence}.balance")).unwrap();
                let local_x = x + offset;
                let expected = local_x * local_x * y;
                assert_eq!(result.last_value(output).unwrap().value(), expected);
                let Some(KernelNode::Relation(relation)) = program.node(relation) else {
                    panic!("ordinary compiled Relation");
                };
                let ir = ScalarOperatorIr::lower(
                    &program.numerical_residuals(relation.id().erase()).unwrap(),
                )
                .unwrap();
                assert_eq!(ir.residual_count(), 1);
                let mut inputs = Vec::new();
                let mut roles = Vec::new();
                let mut parameters = Vec::new();
                for symbol in ir.symbols() {
                    match symbol {
                        SymbolRef::Field(field) => {
                            assert_eq!(field.erase(), output);
                            inputs.push(expected);
                            roles.push(DifferentiationRole::Unknown);
                        }
                        SymbolRef::Parameter(parameter) => {
                            let parameter = parameter.erase();
                            assert!([x_id, y_id].contains(&parameter));
                            inputs.push(program.value(parameter).unwrap().value());
                            roles.push(DifferentiationRole::Parameter);
                            parameters.push(parameter);
                        }
                        _ => {
                            panic!("static aliases must retain only original Parameters and output")
                        }
                    }
                }
                assert_eq!(parameters.len(), 2);
                assert_eq!(ir.evaluate(&inputs).unwrap(), [0.0]);
                let linearized = ir.linearize(&inputs, &roles).unwrap();
                let mut output_cotangent = [f64::NAN];
                let mut gradient = [f64::NAN; 2];
                linearized
                    .vjp(
                        &[1.0],
                        RelationCotangent::Both {
                            unknown: &mut output_cotangent,
                            parameter: &mut gradient,
                        },
                    )
                    .unwrap();
                assert_eq!(output_cotangent, [1.0]);
                let mut direction = [0.0; 2];
                for (index, parameter) in parameters.iter().enumerate() {
                    // For r=output-(x+offset)^2*y, dr/dx=-2*(x+offset)*y and dr/dy=-(x+offset)^2.
                    let expected_gradient = if *parameter == x_id {
                        -2.0 * local_x * y
                    } else {
                        -local_x * local_x
                    };
                    assert_eq!(gradient[index], expected_gradient);
                    direction[index] = if *parameter == x_id { 1.0 } else { 2.0 };
                }
                let mut tangent = [f64::NAN];
                linearized
                    .jvp(
                        RelationTangent::Both {
                            unknown: &[1.0],
                            parameter: &direction,
                        },
                        &mut tangent,
                    )
                    .unwrap();
                assert_eq!(tangent, [1.0 - 2.0 * local_x * y - 2.0 * local_x * local_x]);
            }
        }
    }
}
