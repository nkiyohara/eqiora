use super::*;
use crate::compile;
use eqiora_lang::parse;

#[test]
fn entity_symbols_are_ordered_and_exact() {
    let alpha = Id::<kinds::Field>::new().erase();
    let zeta = Id::<kinds::Parameter>::new().erase();
    let symbols = ModelSymbols::from_map(BTreeMap::from([
        ("zeta".to_owned(), zeta),
        ("alpha".to_owned(), alpha),
    ]));

    assert_eq!(symbols.get("alpha"), Some(alpha));
    assert_eq!(symbols.get("zeta"), Some(zeta));
    assert_eq!(symbols.get("missing"), None);
    assert_eq!(
        symbols.iter().collect::<Vec<_>>(),
        vec![("alpha", alpha), ("zeta", zeta)]
    );
}

#[test]
fn executable_compile_still_requires_a_model_entry() {
    let source = "public component Resistor {} public connector Pin = scalar_physical(across = 1, through = A);";
    let diagnostics = compile("library.eqi", source)
        .expect_err("a declarations-only library is not an executable model");

    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code(), codes::SYNTAX_ERROR);
    assert_eq!(
        diagnostics[0].message(),
        "source requires at least one `model` declaration"
    );
    let span = diagnostics[0]
        .source_span()
        .expect("missing Model remains a source diagnostic");
    assert_eq!(span.file, "library.eqi");
    assert_eq!(span.start, u32::try_from(source.len()).unwrap());
    assert_eq!(span.end, span.start);
}

struct AssignedTestIdentities {
    model: OntologyId<Model>,
    domain: Id<kinds::Domain>,
    ports: BTreeMap<String, Id<kinds::Port>>,
    relation: Id<kinds::Relation>,
    activation: Id<kinds::Activation>,
    connection: Id<kinds::Connection>,
}

impl LoweringIdentities for AssignedTestIdentities {
    fn model(&mut self, _name: &str) -> OntologyId<Model> {
        self.model
    }

    fn domain(&mut self, _name: &str) -> Id<kinds::Domain> {
        self.domain
    }

    fn representation(&mut self, _name: &str) -> Id<kinds::Representation> {
        panic!("fixture has no Representation")
    }

    fn field(&mut self, _name: &str) -> Id<kinds::Field> {
        panic!("fixture has no Field")
    }

    fn parameter(&mut self, _name: &str) -> Id<kinds::Parameter> {
        panic!("fixture has no Parameter")
    }

    fn port(&mut self, name: &str) -> Id<kinds::Port> {
        self.ports[name]
    }

    fn clock(&mut self, _name: &str) -> Id<kinds::ClockDomain> {
        panic!("fixture has no ClockDomain")
    }

    fn relation(&mut self, _name: &str) -> (Id<kinds::Relation>, Id<kinds::Activation>) {
        (self.relation, self.activation)
    }

    fn connection(&mut self) -> Id<kinds::Connection> {
        self.connection
    }
}

fn voltage_dimension() -> DimExponents {
    DimExponents {
        mass: 1,
        length: 2,
        time: -3,
        current: -1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn current_dimension() -> DimExponents {
    DimExponents {
        current: 1,
        ..DimExponents::DIMENSIONLESS
    }
}

fn resistance_dimension() -> DimExponents {
    DimExponents {
        mass: 1,
        length: 2,
        time: -3,
        current: -2,
        ..DimExponents::DIMENSIONLESS
    }
}

#[test]
fn staged_identity_source_controls_every_lowered_identity() {
    let source = r#"
model assigned {
  domain electrical = scalar_physical(across = 1, through = 1);
  port positive: conserving on electrical;
  port negative: conserving on electrical;
  relation equal continuous { across(positive) - across(negative) = 0; }
  connect conserving positive, negative;
}
"#;
    let document = parse("assigned.eqi", source)
        .into_document()
        .expect("fixture parses");
    let positive = Id::new();
    let negative = Id::new();
    let mut identities = AssignedTestIdentities {
        model: OntologyId::new(),
        domain: Id::new(),
        ports: BTreeMap::from([
            ("negative".to_owned(), negative),
            ("positive".to_owned(), positive),
        ]),
        relation: Id::new(),
        activation: Id::new(),
        connection: Id::new(),
    };

    let compiled =
        lower_model_with_identities("assigned.eqi", &document.models()[0], &mut identities)
            .expect("assigned identities lower");

    assert_eq!(compiled.model(), identities.model);
    assert_eq!(
        compiled.symbols().get("electrical"),
        Some(identities.domain.erase())
    );
    assert_eq!(compiled.symbols().get("positive"), Some(positive.erase()));
    assert_eq!(compiled.symbols().get("negative"), Some(negative.erase()));
    assert_eq!(
        compiled.symbols().get("equal"),
        Some(identities.relation.erase())
    );
    assert!(compiled.transaction().ops().iter().any(|operation| {
        matches!(
            operation,
            Op::DefineKernelNode {
                node: KernelNode::Activation(definition),
            } if definition.id() == identities.activation
        )
    }));
    assert!(compiled.transaction().ops().iter().any(|operation| {
        matches!(
            operation,
            Op::DefineKernelNode {
                node: KernelNode::Connection(definition),
            } if definition.id() == identities.connection
        )
    }));
}

#[test]
fn compiler_rejects_dimensionally_invalid_residual_at_source_span() {
    let source = r#"
model invalid {
  field temperature: K = 293;
  parameter tau: s = 10;
  relation bad continuous {
    temperature + tau = 0;
  }
}
"#;
    let diagnostics = compile("invalid.eqi", source).expect_err("K + s is invalid");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::LANGUAGE_TYPE_ERROR && diagnostic.source_span().is_some()
    }));
}

#[test]
fn compiler_rejects_unresolved_periodic_clock() {
    let source = "model m { field x: 1 = 0; relation r periodic(missing) { next(x) = 0; } }";
    let diagnostics = compile("missing.eqi", source).expect_err("clock is unresolved");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("periodic ClockDomain"))
    );
}

#[test]
fn compiler_rejects_discrete_symbols_in_continuous_relations() {
    let source = "model m { field x: 1 = 0; relation r continuous { next(x) = 0; } }";
    let diagnostics = compile("activation.eqi", source).expect_err("Next needs a tick");

    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message().contains("continuous Relation"))
    );
}

#[test]
fn compiler_rejects_spatial_boundary_unit_mismatch() {
    let source = r#"
model bar {
  domain body = box(0, 1);
  domain loaded = boundary(body, axis = 0, side = upper);
  representation space = continuum;
  field u on body as space: m = 0;
  parameter stiffness: kg * m / s ^ 2 = 10;
  parameter wrong_load: m = 1;
  relation load continuous on loaded {
    normal(stiffness * grad(u)) - wrong_load = 0;
  }
}
"#;
    let diagnostics = compile("bar.eqi", source).expect_err("force and length conflict");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
            && diagnostic.message().contains("addition/subtraction")
    }));
}

#[test]
fn compiler_requires_dimensionless_trigonometric_arguments() {
    let source = r#"
model invalid {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: 1 = 0;
  relation balance continuous on interval {
    -div(grad(u)) - math.sin(coordinate(0)) = 0;
  }
}

"#;
    let diagnostics = compile("invalid-sin.eqi", source)
        .expect_err("a physical coordinate is not an angle by itself");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
            && diagnostic.message().contains("dimensionless scalar")
    }));
}

#[test]
fn compiler_owns_the_scalar_mathematics_namespace() {
    let valid = r#"
model valid {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: 1 = 0;
  relation balance continuous on interval {
    u - math.sin(math.pi) = 0;
  }
}
"#;
    let compiled = compile("math-sin.eqi", valid).expect("the compiler-owned sine compiles");
    let relation = compiled[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation),
            _ => None,
        })
        .expect("canonical Relation");
    assert!(relation.residuals().nodes().iter().any(|node| matches!(
        node,
        eqiora_schema::kernel::ExprNode::UnaryMath(UnaryMathFunction::Sin, _)
    )));
    assert!(relation.residuals().nodes().iter().any(|node| matches!(
        node,
        eqiora_schema::kernel::ExprNode::Constant(value)
            if value.value().to_bits() == 0x4009_21fb_5444_2d18
    )));

    for (source, expected) in [
        (
            "model invalid { domain d = box(0, 1); representation r = continuum; field u on d as r: 1 = 0; relation law continuous on d { u - sin(0) = 0; } }",
            "bare `sin` is not language vocabulary",
        ),
        (
            "model invalid { domain d = box(0, 1); representation r = continuum; field u on d as r: 1 = 0; relation law continuous on d { u - math.cos(0) = 0; } }",
            "unknown compiler-owned scalar mathematics member `math.cos`",
        ),
        (
            "model invalid { domain d = box(0, 1); representation r = continuum; field u on d as r: 1 = 0; relation law continuous on d { u - math.tau = 0; } }",
            "unknown compiler-owned scalar mathematics member `math.tau`",
        ),
        (
            "model invalid { parameter math: 1 = 1; }",
            "identifier `math` is reserved for compiler-owned scalar mathematics",
        ),
        (
            "model math { relation law continuous { 0 = 0; } }",
            "identifier `math` is reserved for compiler-owned scalar mathematics",
        ),
        (
            "dimension math = m; model invalid { relation law continuous { 0 = 0; } }",
            "identifier `math` is reserved for compiler-owned scalar mathematics",
        ),
    ] {
        let diagnostics = compile("invalid-math.eqi", source).expect_err(expected);
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "missing diagnostic containing {expected:?}: {diagnostics:#?}"
        );
    }
}

#[test]
fn compiler_lowers_canonical_tensor_structure_without_a_physics_node() {
    let source = r#"
model elastic_relation {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field displacement on body as space: m shape spatial_vector;
  parameter mu: kg / (m * s ^ 2) = 2;
  parameter lambda: kg / (m * s ^ 2) = 3;
  relation balance continuous on body {
    -div(
      2 * mu * symmetric_part(grad(displacement))
      + lambda * isotropic_lift(div(displacement))
    ) = 0;
  }
}
"#;
    let compiled = compile("elastic-relation.eqi", source).expect("typed tensor relation");
    let relation = compiled[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation),
            _ => None,
        })
        .expect("canonical Relation");
    assert!(
        relation
            .residuals()
            .nodes()
            .iter()
            .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::SymmetricPart(_)))
    );
    assert!(
        relation
            .residuals()
            .nodes()
            .iter()
            .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::IsotropicLift(_)))
    );
}

#[test]
fn flat_semantic_typing_distinguishes_scalar_gradients_from_vector_strain() {
    let scalar = r#"
model scalar_poisson {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field potential on body as space: 1;
  relation balance continuous on body { -div(grad(potential)) = 0; }
}
"#;
    compile("scalar-poisson.eqi", scalar).expect("a scalar gradient remains admissible");

    let wrong_displacement = scalar
        .replace(
            "field potential on body as space: 1;",
            "field potential on body as space: 1 shape scalar;",
        )
        .replace("-div(grad(potential))", "symmetric_part(grad(potential))");
    let diagnostics = compile("wrong-strain.eqi", &wrong_displacement)
        .expect_err("symmetric strain requires a spatial-vector Field");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
            && diagnostic
                .message()
                .contains("symmetric_part requires an exact")
    }));
}

#[test]
fn compiler_lowers_source_declared_pure_operator_as_one_generic_application() {
    let source = r#"
public pure operator dyadic(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);

model generic_operator {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field left on body as space: 1 shape spatial_vector;
  field right on body as space: 1 shape spatial_vector;
  relation balance continuous on body {
    div(div(dyadic(left, right))) = 0;
  }
}
"#;
    let compiled = compile("generic-operator.eqi", source).expect("typed pure operator");
    let relation = compiled[0]
        .transaction()
        .ops()
        .iter()
        .find_map(|operation| match operation {
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => Some(relation),
            _ => None,
        })
        .expect("canonical Relation");
    let dag = relation.residuals();
    let applications = dag
        .nodes()
        .iter()
        .filter_map(|node| match node {
            eqiora_schema::kernel::ExprNode::PureOperatorApplication(application) => {
                Some(application)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(applications.len(), 1);
    assert_eq!(applications[0].arguments().len(), 2);
    assert_eq!(dag.definitions().len(), 1);
    assert_eq!(
        applications[0].definition(),
        eqiora_schema::kernel::pure_operator::PureOperatorDefinition::dyadic_product()
            .expect("standard definition")
            .digest()
    );
    let (transaction, _, _) = compiled
        .into_iter()
        .next()
        .expect("compiled Model")
        .into_parts();
    eqiora_graph::GraphStore::commit(&mut eqiora_graph::InMemoryGraphStore::new(), transaction)
        .expect("generic application passes whole-model admission");
}

#[test]
fn pure_operator_arity_and_exact_value_class_fail_before_lowering() {
    let prefix = r#"
public pure operator dyadic(left: spatial[1], right: spatial[1]) -> spatial[2]
  = component(left, 0) * component(right, 1);
model invalid {
  domain body = box(0, 1, 0, 1);
  representation space = continuum;
  field scalar on body as space: 1 = 0;
  field vector on body as space: 1 shape spatial_vector;
  relation balance continuous on body {
"#;
    for (residual, expected) in [
        ("dyadic(vector) = 0;", "argument count"),
        ("dyadic(scalar, vector) = 0;", "exact type rule"),
    ] {
        let source = format!("{prefix}{residual}\n  }}\n}}\n");
        let diagnostics = compile("invalid-pure-operator.eqi", &source)
            .expect_err("invalid application must fail closed");
        assert!(
            diagnostics
                .iter()
                .any(|diagnostic| diagnostic.message().contains(expected)),
            "expected `{expected}`, got {diagnostics:#?}"
        );
    }
}

#[test]
fn compiler_requires_a_literal_coordinate_axis() {
    let source = r#"
model invalid {
  domain interval = box(0, 1);
  representation space = continuum;
  field u on interval as space: m = 0;
  relation identity continuous on interval { u - coordinate(u) = 0; }
}
"#;
    let diagnostics = compile("invalid-coordinate.eqi", source)
        .expect_err("coordinate axis selection must remain structural");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.code() == codes::LANGUAGE_TYPE_ERROR
            && diagnostic.message().contains("integer literal axis")
    }));
}

#[test]
fn native_lowering_replaces_synthetic_ranges_with_declaration_paths() {
    let temperature = eqiora_lang::DraftField::new(
        "temperature",
        DimExponents {
            temperature: 1,
            ..DimExponents::DIMENSIONLESS
        },
        293.0,
    );
    let duration = eqiora_lang::DraftParameter::new(
        "duration",
        DimExponents {
            time: 1,
            ..DimExponents::DIMENSIONLESS
        },
        1.0,
    );
    let relation = eqiora_lang::DraftRelation::continuous(
        "invalid",
        [temperature.expression() + duration.expression()],
    );
    let draft = ModelDraft::new(
        "thermal",
        [temperature.into(), duration.into(), relation.into()],
    )
    .unwrap();

    let diagnostics = lower_draft(&draft).unwrap_err();
    assert_eq!(diagnostics[0].code(), codes::LANGUAGE_TYPE_ERROR);
    assert_eq!(
        diagnostics[0].graph_path().unwrap().to_string(),
        "thermal.invalid"
    );
    assert!(diagnostics[0].source_span().is_none());
}

#[test]
fn source_and_native_physical_models_lower_to_the_same_normalized_semantics() {
    let source = r#"
model resistor {
  domain electrical = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port positive: conserving on electrical;
  port negative: conserving on electrical;
  port tap: conserving on electrical;
  parameter resistance: kg * m ^ 2 / (s ^ 3 * A ^ 2) = 2;
  relation law continuous {
    across(positive) - across(negative) - resistance * through(positive) = 0;
    through(positive) + through(negative) + through(tap) = 0;
  }
  connect conserving positive, negative, tap;
}
"#;
    let source_model = compile("resistor.eqi", source).unwrap().remove(0);

    let electrical = eqiora_lang::DraftPhysicalDomain::new(
        "electrical",
        voltage_dimension(),
        current_dimension(),
    );
    let positive = eqiora_lang::DraftConservingPort::new("positive", &electrical);
    let negative = eqiora_lang::DraftConservingPort::new("negative", &electrical);
    let tap = eqiora_lang::DraftConservingPort::new("tap", &electrical);
    let resistance = eqiora_lang::DraftParameter::new("resistance", resistance_dimension(), 2.0);
    let law = eqiora_lang::DraftRelation::continuous(
        "law",
        [
            eqiora_lang::DraftExpression::across(&positive)
                - eqiora_lang::DraftExpression::across(&negative)
                - resistance.expression() * eqiora_lang::DraftExpression::through(&positive),
            eqiora_lang::DraftExpression::through(&positive)
                + eqiora_lang::DraftExpression::through(&negative)
                + eqiora_lang::DraftExpression::through(&tap),
        ],
    );
    let draft = ModelDraft::new(
        "resistor",
        [
            electrical.into(),
            positive.clone().into(),
            negative.clone().into(),
            tap.clone().into(),
            resistance.into(),
            law.into(),
            eqiora_lang::DraftConservingConnection::new([&positive, &negative, &tap]).into(),
        ],
    )
    .unwrap();
    let native_model = lower_draft(&draft).unwrap();

    assert_eq!(
        normalized_physical_semantics(&source_model),
        normalized_physical_semantics(&native_model)
    );

    let (transaction, _, _) = native_model.into_parts();
    let mut store = eqiora_graph::InMemoryGraphStore::new();
    eqiora_graph::GraphStore::commit(&mut store, transaction)
        .expect("the shared compiler transaction must pass full graph admission");
}

#[test]
fn native_physical_projection_is_insensitive_to_declaration_and_net_permutation() {
    let electrical = eqiora_lang::DraftPhysicalDomain::new(
        "electrical",
        voltage_dimension(),
        current_dimension(),
    );
    let positive = eqiora_lang::DraftConservingPort::new("positive", &electrical);
    let negative = eqiora_lang::DraftConservingPort::new("negative", &electrical);
    let relation = eqiora_lang::DraftRelation::continuous(
        "balance",
        [
            eqiora_lang::DraftExpression::across(&positive)
                - eqiora_lang::DraftExpression::across(&negative),
            eqiora_lang::DraftExpression::through(&positive)
                + eqiora_lang::DraftExpression::through(&negative),
        ],
    );
    let forward = ModelDraft::new(
        "permuted",
        [
            electrical.clone().into(),
            positive.clone().into(),
            negative.clone().into(),
            relation.clone().into(),
            eqiora_lang::DraftConservingConnection::new([&positive, &negative]).into(),
        ],
    )
    .unwrap();
    let reversed = ModelDraft::new(
        "permuted",
        [
            eqiora_lang::DraftConservingConnection::new([&negative, &positive]).into(),
            relation.into(),
            negative.into(),
            positive.into(),
            electrical.into(),
        ],
    )
    .unwrap();

    assert_eq!(
        normalized_physical_semantics(&lower_draft(&forward).unwrap()),
        normalized_physical_semantics(&lower_draft(&reversed).unwrap())
    );
}

#[test]
fn direct_flat_physical_fragments_normalize_before_kernel_lowering() {
    let nary = r#"
model network {
  domain physical = scalar_physical(across = 1, through = 1);
  port a: conserving on physical;
  port b: conserving on physical;
  port c: conserving on physical;
  relation owner continuous {
    across(a) - across(b) = 0;
    across(b) - across(c) = 0;
    through(a) + through(b) + through(c) = 0;
  }
  connect conserving a, b, c;
}
"#;
    let chain = r#"
model network {
  domain physical = scalar_physical(across = 1, through = 1);
  port a: conserving on physical;
  port b: conserving on physical;
  port c: conserving on physical;
  relation owner continuous {
    across(a) - across(b) = 0;
    across(b) - across(c) = 0;
    through(a) + through(b) + through(c) = 0;
  }
  connect conserving a, b;
  connect conserving b, c;
}
"#;
    let nary = compile("nary.eqi", nary).unwrap().remove(0);
    let chain = compile("chain.eqi", chain).unwrap().remove(0);

    assert_eq!(
        normalized_physical_semantics(&nary),
        normalized_physical_semantics(&chain)
    );
    assert_eq!(
        chain
            .transaction()
            .ops()
            .iter()
            .filter(|operation| matches!(
                operation,
                Op::DefineKernelNode {
                    node: KernelNode::Connection(_),
                }
            ))
            .count(),
        1
    );

    let (transaction, _, _) = chain.into_parts();
    let mut store = eqiora_graph::InMemoryGraphStore::new();
    eqiora_graph::GraphStore::commit(&mut store, transaction).unwrap();
}

#[test]
fn compiler_preserves_legacy_conserving_markers() {
    let source = r#"
model legacy {
  port p: conserving A;
  relation owner continuous { p = 0; }
}
"#;
    let models = compile("legacy.eqi", source).expect("legacy marker remains source-valid");
    let port = models[0].symbols().get("p").expect("Port ID");
    let relation = models[0].symbols().get("owner").expect("Relation ID");
    let mut saw_marker = false;
    let mut saw_legacy_symbol = false;
    for operation in models[0].transaction().ops() {
        let Op::DefineKernelNode { node } = operation else {
            continue;
        };
        match node {
            KernelNode::Port(definition) if definition.id().erase() == port => {
                saw_marker = definition.marker_dimension().is_some();
            }
            KernelNode::Relation(definition) if definition.id().erase() == relation => {
                saw_legacy_symbol = definition
                        .residuals()
                        .nodes()
                        .iter()
                        .any(|node| matches!(node, eqiora_schema::kernel::ExprNode::Symbol(SymbolRef::Port(id)) if id.erase() == port));
            }
            _ => {}
        }
    }
    assert!(saw_marker);
    assert!(saw_legacy_symbol);
}

#[test]
fn compiler_rejects_dimension_coincidence_across_nominal_domains() {
    let source = r#"
model crossed_types {
  domain electrical_a = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  domain electrical_b = scalar_physical(across = kg * m ^ 2 / (s ^ 3 * A), through = A);
  port a: conserving on electrical_a;
  port b: conserving on electrical_b;
  relation owner_a continuous { across(a) = 0; }
  relation owner_b continuous { across(b) = 0; }
  connect conserving a, b;
}
"#;
    let diagnostics = compile("crossed-types.eqi", source)
        .expect_err("equal dimensions never erase nominal Domain identity");

    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.source_span().is_some()
            && diagnostic.message().contains("exact same nominal Domain")
    }));
}

#[test]
fn flat_lowering_consumes_the_shared_scalar_connection_contract() {
    let cases = [
        (
            "dimension mismatch",
            "model m { port out: signal output m; port sink: signal input s; connect signal out -> sink; }",
            "dimension-matched inputs",
        ),
        (
            "source direction",
            "model m { port out: signal output 1; port sink: signal input 1; connect signal sink -> out; }",
            "source before `->`",
        ),
        (
            "mixed conserving families",
            "model m { domain d = scalar_physical(across = 1, through = 1); port marker: conserving 1; port physical: conserving on d; connect conserving marker, physical; }",
            "cannot mix",
        ),
    ];
    for (name, source, message) in cases {
        let document = parse("connections.eqi", source)
            .into_document()
            .expect("fixture parses");
        let diagnostics = lower_model("connections.eqi", &document.models()[0]).expect_err(name);
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.source_span().is_some() && diagnostic.message().contains(message)
            }),
            "{name}: {diagnostics:?}"
        );
    }
}

#[test]
fn compiler_rejects_non_physical_domains_and_unqualified_physical_ports() {
    let wrong_domain = r#"
model wrong_domain {
  domain space = box(0, 1);
  port p: conserving on space;
  relation owner continuous { across(p) = 0; }
}
"#;
    let diagnostics = compile("wrong-domain.eqi", wrong_domain)
        .expect_err("spatial Domains cannot type physical Ports");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.source_span().is_some() && diagnostic.message().contains("not scalar physical")
    }));

    let unqualified = r#"
model unqualified {
  domain electrical = scalar_physical(across = 1, through = 1);
  port p: conserving on electrical;
  relation owner continuous { p = 0; }
}
"#;
    let diagnostics = compile("unqualified.eqi", unqualified)
        .expect_err("physical variables require an explicit accessor");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.source_span().is_some()
            && diagnostic.message().contains("must be read as `across(p)`")
    }));
}

#[test]
fn physical_accessors_require_one_bare_physical_port_name() {
    let malformed = r#"
model malformed {
  domain electrical = scalar_physical(across = 1, through = 1);
  port p: conserving on electrical;
  relation owner continuous { across(p + 1) = 0; }
}
"#;
    let diagnostics =
        compile("malformed.eqi", malformed).expect_err("accessor structure remains explicit");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.source_span().is_some()
            && diagnostic
                .message()
                .contains("one bare scalar physical Port name")
    }));

    let signal = r#"
model signal_accessor {
  port p: signal input 1;
  relation owner continuous { through(p) = 0; }
}
"#;
    let diagnostics =
        compile("signal-accessor.eqi", signal).expect_err("signal Ports have no through variable");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.source_span().is_some()
            && diagnostic.message().contains("not a scalar physical Port")
    }));
}

fn normalized_physical_semantics(model: &CompiledModel) -> Vec<String> {
    use eqiora_schema::kernel::{ActivationKind, DomainKind, ExprNode, PortPayload, SymbolRef};

    let names = model
        .symbols()
        .iter()
        .map(|(name, id)| (id, name.to_owned()))
        .collect::<BTreeMap<_, _>>();
    let mut signatures = Vec::new();
    let mut connections = BTreeMap::<RawId, Vec<String>>::new();
    let mut activations = BTreeMap::new();

    for operation in model.transaction().ops() {
        match operation {
            Op::DefineKernelNode {
                node: KernelNode::Domain(domain),
            } => {
                if let DomainKind::ScalarPhysical {
                    across_dimension,
                    through_dimension,
                } = domain.kind()
                {
                    signatures.push(format!(
                        "domain:{}:{across_dimension:?}:{through_dimension:?}",
                        named(&names, domain.id().erase())
                    ));
                }
            }
            Op::DefineKernelNode {
                node: KernelNode::Parameter(parameter),
            } => signatures.push(format!(
                "parameter:{}:{:016x}:{:?}",
                named(&names, parameter.id().erase()),
                parameter.value().value().to_bits(),
                parameter.value().dim()
            )),
            Op::DefineKernelNode {
                node: KernelNode::Port(port),
            } => {
                if let PortPayload::ScalarPhysical { domain } = port.payload() {
                    signatures.push(format!(
                        "port:{}:{}",
                        named(&names, port.id().erase()),
                        named(&names, domain.erase())
                    ));
                }
            }
            Op::DefineKernelNode {
                node: KernelNode::Relation(relation),
            } => {
                signatures.push(format!(
                    "relation:{}:{}",
                    named(&names, relation.id().erase()),
                    normalize_dag(relation.residuals(), &names)
                ));
            }
            Op::DefineKernelNode {
                node: KernelNode::Activation(activation),
            } => {
                let kind = match activation.kind() {
                    ActivationKind::Continuous => "continuous",
                    ActivationKind::Periodic => "periodic",
                    ActivationKind::Event { .. } => "event",
                    ActivationKind::Guard { .. } => "guard",
                    _ => "newer",
                };
                activations.insert(activation.id().erase(), kind);
            }
            Op::DefineKernelNode {
                node: KernelNode::Connection(connection),
            } => {
                signatures.push(format!("connection-kind:{:?}", connection.semantics()));
                connections.entry(connection.id().erase()).or_default();
            }
            Op::Connect {
                from,
                to,
                edge: EdgeKind::Connects,
            } => connections
                .entry(*from)
                .or_default()
                .push(named(&names, *to).to_owned()),
            Op::Connect {
                from,
                to,
                edge: EdgeKind::DependsOn | EdgeKind::HasPort,
            } => signatures.push(format!(
                "edge:{:?}:{}:{}",
                operation_edge(operation),
                named(&names, *from),
                named(&names, *to)
            )),
            Op::Connect {
                from,
                to,
                edge: EdgeKind::Activates,
            } => signatures.push(format!(
                "activation:{}:{}",
                activations.get(from).copied().unwrap_or("missing"),
                named(&names, *to)
            )),
            _ => {}
        }
    }

    for mut members in connections.into_values() {
        members.sort();
        signatures.push(format!("connection-members:{members:?}"));
    }
    signatures.sort();

    fn normalize_dag(
        dag: &eqiora_schema::kernel::ExprDag,
        names: &BTreeMap<RawId, String>,
    ) -> String {
        let nodes = dag
            .nodes()
            .iter()
            .map(|node| match node {
                ExprNode::Constant(value) => format!(
                    "constant({:016x},{:?})",
                    value.value().to_bits(),
                    value.dim()
                ),
                ExprNode::Symbol(symbol) => normalize_symbol(*symbol, names),
                ExprNode::Neg(value) => format!("neg({})", value.index()),
                ExprNode::Add(left, right) => {
                    format!("add({},{})", left.index(), right.index())
                }
                ExprNode::Sub(left, right) => {
                    format!("sub({},{})", left.index(), right.index())
                }
                ExprNode::Mul(left, right) => {
                    format!("mul({},{})", left.index(), right.index())
                }
                ExprNode::Div(left, right) => {
                    format!("div({},{})", left.index(), right.index())
                }
                other => format!("{other:?}"),
            })
            .collect::<Vec<_>>();
        let roots = dag
            .roots()
            .iter()
            .map(|root| root.index())
            .collect::<Vec<_>>();
        format!("{nodes:?}:{roots:?}")
    }

    fn normalize_symbol(symbol: SymbolRef, names: &BTreeMap<RawId, String>) -> String {
        match symbol {
            SymbolRef::Field(id) => format!("field({})", named(names, id.erase())),
            SymbolRef::Derivative(id) => {
                format!("derivative({})", named(names, id.erase()))
            }
            SymbolRef::Pre(id) => format!("pre({})", named(names, id.erase())),
            SymbolRef::Next(id) => format!("next({})", named(names, id.erase())),
            SymbolRef::Parameter(id) => format!("parameter({})", named(names, id.erase())),
            SymbolRef::Port(id) => format!("port({})", named(names, id.erase())),
            SymbolRef::Across(id) => format!("across({})", named(names, id.erase())),
            SymbolRef::Through(id) => format!("through({})", named(names, id.erase())),
            SymbolRef::Time => "time".to_owned(),
            _ => "newer-symbol".to_owned(),
        }
    }

    fn operation_edge(operation: &Op) -> EdgeKind {
        let Op::Connect { edge, .. } = operation else {
            unreachable!("only Connect operations reach this helper");
        };
        *edge
    }

    fn named(names: &BTreeMap<RawId, String>, id: RawId) -> &str {
        names.get(&id).map_or("<anonymous>", String::as_str)
    }

    signatures
}
