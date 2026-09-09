use eqiora_compiler::{CompiledModel, compile};
use eqiora_graph::Op;
use eqiora_schema::kernel::{ExprNode, KernelNode, SymbolRef};

fn accepted(source: &str) -> Vec<CompiledModel> {
    compile("named.eqi", source).unwrap_or_else(|errors| panic!("{source}\n{errors:?}"))
}

fn scalar_roles(models: &[CompiledModel]) -> (usize, usize) {
    let mut result = (0, 0);
    for model in models {
        for op in model.transaction().ops() {
            if let Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } = op
            {
                for node in relation.expression().nodes() {
                    match node {
                        ExprNode::Symbol(SymbolRef::Across(_)) => result.0 += 1,
                        ExprNode::Symbol(SymbolRef::Through(_)) => result.1 += 1,
                        _ => {}
                    }
                }
            }
        }
    }
    result
}

const WIRE: &str = "connector Pin { across voltage: V; through current: A; }
component Wire(port p: Pin, port n: Pin) { relation law { p.voltage=n.voltage; p.current+n.current=0; } }
model Circuit() { instance a:Wire(); instance b:Wire(); connect a.p,b.p; connect a.n,b.n; }";

#[test]
fn scalar_quantity_rename_preserves_roles_not_lookup_spelling() {
    assert_eq!(scalar_roles(&accepted(WIRE)), (4, 4));
    let renamed = WIRE
        .replace("voltage", "potential")
        .replace("current", "flow");
    assert_eq!(scalar_roles(&accepted(&renamed)), (4, 4));
    let stale = renamed.replace("p.potential", "p.voltage");
    let errors = compile("stale.eqi", &stale).unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("no declared quantity `voltage`"))
    );
}

#[test]
fn native_domain_projection_uses_the_same_declared_member_frontdoor() {
    use eqiora_core::{DimExponents, ScalarDomain, ValueType};
    use eqiora_lang::{
        DraftConservingConnection, DraftConservingPort, DraftExpression, DraftPhysicalDomain,
        DraftRelation, Module,
    };
    let scalar = ValueType::scalar(ScalarDomain::Real, DimExponents::DIMENSIONLESS).unwrap();
    let domain = DraftPhysicalDomain::new("physical", "potential", scalar.clone(), "flow", scalar);
    let p = DraftConservingPort::new("p", &domain);
    let n = DraftConservingPort::new("n", &domain);
    let relation = DraftRelation::continuous(
        "law",
        [
            (DraftExpression::across(&p), DraftExpression::across(&n)),
            (
                DraftExpression::through(&p) + DraftExpression::through(&n),
                DraftExpression::constant(eqiora_lang::DecimalLiteral::parse("0").unwrap()),
            ),
        ],
    );
    let connection = DraftConservingConnection::new([&p, &n]);
    let draft = Module::new(
        "M",
        [
            domain.into(),
            p.into(),
            n.into(),
            relation.into(),
            connection.into(),
        ],
    )
    .unwrap();
    let model = eqiora_compiler::lower_module(&draft, None, &[]).unwrap();
    assert_eq!(scalar_roles(&[model]), (2, 2));
}

#[test]
fn names_do_not_replace_nominal_connector_compatibility() {
    let source =
        "connector A { across v: V; through i: A; } connector B { across v: V; through i: A; }
component Left(port p:A) {relation r {p.v=0; p.i=0;}}
component Right(port p:B) {relation r {p.v=0; p.i=0;}}
model M() {instance a:Left();instance b:Right();connect a.p,b.p;}";
    assert!(compile("nominal.eqi", source).is_err());
}

#[test]
fn physical_role_call_spellings_are_not_source_builtins() {
    for replacement in ["across(p)", "through(p)", "flux(p)", "p.missing"] {
        assert!(
            compile("retired.eqi", &WIRE.replace("p.voltage", replacement)).is_err(),
            "{replacement}"
        );
    }
}

#[test]
fn trace_and_flux_names_use_existing_exact_boundary_types_in_2d_and_3d() {
    for dimension in [2, 3] {
        let coordinates = if dimension == 2 {
            "0,1,0,1"
        } else {
            "0,1,0,1,0,1"
        };
        let neighbor = if dimension == 2 {
            "1,2,0,1"
        } else {
            "1,2,0,1,0,1"
        };
        let source = format!("connector Mechanical {{ trace velocity:m/s; flux traction:Pa; shape spatial_vector; frame spatial; pairing euclidean_boundary_duality; orientation parent_outward; }}
component Wall(support body:volume(ambient_dimension={dimension}),support face:boundary(parent=body),port mechanical:Mechanical over face) {{relation no_slip on face {{mechanical.velocity=0;}}}}
model M() {{domain body=box({coordinates});domain face=boundary(body,axis=0,side=upper);domain other=box({neighbor});domain interface=boundary(other,axis=0,side=lower);instance wall:Wall(body=body,face=face);instance neighbor:Wall(body=other,face=interface);connect wall.mechanical,neighbor.mechanical;}}");
        let models = accepted(&source);
        assert!(models[0].transaction().ops().iter().any(|op| matches!(op,
            Op::DefineKernelNode {node:KernelNode::Relation(relation)} if relation.expression().nodes().iter().any(|node| matches!(node,ExprNode::Symbol(SymbolRef::PortTrace(_)))))));
        assert!(
            compile(
                "old-trace.eqi",
                &source.replace("mechanical.velocity", "trace(mechanical)")
            )
            .is_err()
        );
        assert!(
            compile(
                "wrong-role.eqi",
                &source.replace("mechanical.velocity", "mechanical.voltage")
            )
            .is_err()
        );
    }
}
