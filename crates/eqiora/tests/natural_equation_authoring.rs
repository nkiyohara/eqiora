use eqiora::api::ModelDocument;
use eqiora::kernel::{ExprDag, ExprNode, KernelNode, SymbolRef};
use eqiora::language::{BinaryOp, Expr, ExprKind, Item, UnaryOp, format, parse};
use eqiora::language::{DraftField, DraftParameter, DraftRelation, ModelDraft};
use eqiora::package::{
    AuthorManifestV1, AuthorPackageSourcesV1, BundleEntryV1, BundleRoleV1, ExactVersion,
    InMemoryPackageStore, NormalizedRelativePath, PackageCompilationRecordV1, PackageReleaseV1,
    PackagedModelDocument, QualifiedName, ResolutionRecordV1, SourceFileV1,
    prepare_package_release_v1,
};
use eqiora::{DimExponents, Id, kinds};

const NATURAL: &str =
    include_str!("../../../verify/language/natural-equation-authoring/models/natural.eqi");
const EXPLICIT: &str = include_str!(
    "../../../verify/language/natural-equation-authoring/models/explicit-residual.eqi"
);
const EXPECTED_CANONICAL: &str = NATURAL;
const EXPECTED_FINGERPRINT: &str =
    "e7662b36983484f8385eb4d5f595f86c87d6aed3e4c6276c569049dc033e9cc5";
const UNDERFLOW_ZERO: &str = "1e-400";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    Number(u64),
    Name(String),
    Path(String),
    Neg(Box<Self>),
    Binary(BinaryOp, Box<Self>, Box<Self>),
    Call(String, Vec<Self>),
}

impl Shape {
    fn name(value: &str) -> Self {
        Self::Name(value.to_owned())
    }

    fn number(value: f64) -> Self {
        Self::Number(value.to_bits())
    }

    fn neg(value: Self) -> Self {
        Self::Neg(Box::new(value))
    }

    fn binary(op: BinaryOp, left: Self, right: Self) -> Self {
        Self::Binary(op, Box::new(left), Box::new(right))
    }

    fn sub(left: Self, right: Self) -> Self {
        Self::binary(BinaryOp::Sub, left, right)
    }
}

#[derive(Debug, PartialEq, Eq)]
struct DiagnosticProjection {
    code: &'static str,
    message_class: String,
    graph_path: Option<Vec<String>>,
    span: Option<(u32, u32)>,
}

struct PositivePair {
    natural: ModelDocument,
    explicit: ModelDocument,
}

#[test]
fn natural_equation_authoring_contract() {
    // The ordinary public nonzero path is intentionally the first evidence
    // action. No rejection or mutant is consulted before this succeeds.
    let pair = ordinary_positive_pair();

    assert_ordered_source_and_kernel_subtraction(&pair);
    assert_formatter_roundtrip(&pair);
    assert_statement_and_operand_order();
    assert_zero_sentinel_and_collision_escape();
    assert_independent_side_precedence();
    assert_parser_owned_ranges();
    assert_fail_closed_order_and_baseline_diagnostics();
    assert_exact_locked_package_path(&pair.explicit);
    assert_native_explicit_residual(&pair.natural);
}

fn ordinary_positive_pair() -> PositivePair {
    let natural = ModelDocument::compile("equation.eqi", NATURAL)
        .expect("ordinary nonzero natural equality must compile first");
    let explicit = ModelDocument::compile("equation.eqi", EXPLICIT)
        .expect("equivalent explicit residual must compile");

    assert!(natural.structurally_equivalent(&explicit).unwrap());
    let natural_fingerprint = natural.structural_fingerprint().unwrap();
    let explicit_fingerprint = explicit.structural_fingerprint().unwrap();
    assert_eq!(natural_fingerprint, explicit_fingerprint);
    assert_eq!(natural_fingerprint.digest(), EXPECTED_FINGERPRINT);

    // Fresh occurrence artifacts are deliberately not the semantic oracle.
    assert_ne!(natural.aliases()["lhs"], explicit.aliases()["lhs"]);
    assert_ne!(
        natural.artifact_reference().unwrap(),
        explicit.artifact_reference().unwrap()
    );
    assert_ne!(
        natural.canonical_json().unwrap(),
        explicit.canonical_json().unwrap()
    );
    assert_ne!(natural.digest().unwrap(), explicit.digest().unwrap());

    PositivePair { natural, explicit }
}

fn assert_ordered_source_and_kernel_subtraction(pair: &PositivePair) {
    let natural_document = parsed("natural.eqi", NATURAL);
    let explicit_document = parsed("explicit-residual.eqi", EXPLICIT);
    let expected = Shape::sub(Shape::name("lhs"), Shape::name("rhs"));
    assert_eq!(
        relation_shapes(&natural_document, "balance"),
        vec![expected.clone()]
    );
    assert_eq!(
        relation_shapes(&explicit_document, "balance"),
        vec![expected.clone()]
    );

    for model in [&pair.natural, &pair.explicit] {
        let relation: Id<kinds::Relation> = model.aliases()["balance"]
            .downcast()
            .expect("typed Relation alias");
        let KernelNode::Relation(definition) = model
            .program()
            .node(relation.erase())
            .expect("accepted Relation")
        else {
            panic!("balance must remain a Relation");
        };
        assert_ordered_sub_dag(definition.residuals(), model);
    }

    for mutant in [
        Shape::sub(Shape::name("rhs"), Shape::name("lhs")),
        Shape::binary(BinaryOp::Add, Shape::name("lhs"), Shape::name("rhs")),
        Shape::name("lhs"),
        Shape::neg(Shape::sub(Shape::name("lhs"), Shape::name("rhs"))),
    ] {
        assert_ne!(expected, mutant, "ordered subtraction mutant survived");
    }
}

fn assert_ordered_sub_dag(dag: &ExprDag, model: &ModelDocument) {
    assert_eq!(dag.nodes().len(), 3, "two operands plus one Sub");
    assert_eq!(dag.roots().len(), 1);
    let ExprNode::Symbol(SymbolRef::Field(lhs)) = dag.nodes()[0] else {
        panic!("lhs must be visited first as a Field symbol");
    };
    let ExprNode::Symbol(SymbolRef::Parameter(rhs)) = dag.nodes()[1] else {
        panic!("rhs must be visited second as a Parameter symbol");
    };
    assert_eq!(lhs.erase(), model.aliases()["lhs"]);
    assert_eq!(rhs.erase(), model.aliases()["rhs"]);
    let ExprNode::Sub(left, right) = dag.nodes()[2] else {
        panic!("the root must be the existing Sub node");
    };
    assert_eq!((left.index(), right.index()), (0, 1));
    assert_eq!(dag.roots()[0].index(), 2);
}

fn assert_formatter_roundtrip(pair: &PositivePair) {
    let natural = parsed("natural.eqi", NATURAL);
    let explicit = parsed("explicit-residual.eqi", EXPLICIT);
    assert_eq!(format(&natural), EXPECTED_CANONICAL);
    assert_eq!(format(&explicit), EXPECTED_CANONICAL);

    let reparsed = parsed("formatted.eqi", &format(&natural));
    assert_eq!(
        relation_shapes(&natural, "balance"),
        relation_shapes(&reparsed, "balance")
    );
    assert_eq!(format(&reparsed), EXPECTED_CANONICAL);
    assert!(
        pair.natural
            .structurally_equivalent(&pair.explicit)
            .unwrap()
    );

    for mutant in [
        EXPECTED_CANONICAL.replace("lhs = rhs;", "rhs = lhs;"),
        EXPECTED_CANONICAL.replace("lhs = rhs;", "lhs + rhs = 0;"),
        EXPECTED_CANONICAL.replace("lhs = rhs;", "lhs = 0;"),
        EXPECTED_CANONICAL.replace("lhs = rhs;", "lhs - rhs = 0;"),
    ] {
        assert_ne!(format(&natural), mutant, "formatter mutant survived");
    }
}

fn assert_statement_and_operand_order() {
    let natural = "model m { field a: 1 = 1; field b: 1 = 2; field c: 1 = 3; field d: 1 = 4; relation r continuous { a = b; c = d; } }";
    let explicit = "model m { field a: 1 = 1; field b: 1 = 2; field c: 1 = 3; field d: 1 = 4; relation r continuous { a - b = 0; c - d = 0; } }";
    let natural_document = parsed("ordered-natural.eqi", natural);
    let explicit_document = parsed("ordered-explicit.eqi", explicit);
    let expected = vec![
        Shape::sub(Shape::name("a"), Shape::name("b")),
        Shape::sub(Shape::name("c"), Shape::name("d")),
    ];
    assert_eq!(relation_shapes(&natural_document, "r"), expected);
    assert_eq!(relation_shapes(&explicit_document, "r"), expected);
    assert_eq!(formatted_statements(natural), ["a = b;", "c = d;"]);

    let natural_model = ModelDocument::compile("ordered.eqi", natural).unwrap();
    let explicit_model = ModelDocument::compile("ordered.eqi", explicit).unwrap();
    assert!(
        natural_model
            .structurally_equivalent(&explicit_model)
            .unwrap()
    );

    let reordered = vec![expected[1].clone(), expected[0].clone()];
    let swapped_inside = vec![
        Shape::sub(Shape::name("b"), Shape::name("a")),
        expected[1].clone(),
    ];
    assert_ne!(expected, reordered);
    assert_ne!(expected, swapped_inside);
}

fn assert_zero_sentinel_and_collision_escape() {
    let underflow = UNDERFLOW_ZERO.parse::<f64>().expect("finite f64 syntax");
    assert!(underflow.is_finite() && underflow == 0.0);

    for statement in ["force = 0;", "force = -0;", "force = 1e-400;"] {
        let source = format!(
            "model m {{ field force: kg * m / s ^ 2 = 1; relation r continuous {{ {statement} }} }}"
        );
        let document = parsed("sentinel.eqi", &source);
        assert_eq!(relation_shapes(&document, "r"), [Shape::name("force")]);
        assert_eq!(formatted_statements(&source), ["force = 0;"]);
        ModelDocument::compile("sentinel.eqi", &source)
            .expect("dimensionful signed-zero sentinel stays accepted");
    }

    let zero = Shape::sub(Shape::name("x"), Shape::number(0.0));
    for statement in ["x = (0);", "x = (1e-400);", "x - 0 = 0;", "x - 1e-400 = 0;"] {
        assert_zero_case(statement, zero.clone(), "x = (0);");
    }
    let negative_zero = Shape::sub(Shape::name("x"), Shape::neg(Shape::number(0.0)));
    for statement in ["x = (-0);", "x - (-0) = 0;"] {
        assert_zero_case(statement, negative_zero.clone(), "x = (-0);");
    }

    let sentinel = parse_shape_for_statement("x = 0;");
    assert_eq!(sentinel, Shape::name("x"));
    assert_ne!(sentinel, zero);
    assert_ne!(sentinel, negative_zero);
    assert_ne!(zero, negative_zero);

    for (statement, expected) in [
        ("x = 1;", Shape::sub(Shape::name("x"), Shape::number(1.0))),
        (
            "x = -1;",
            Shape::sub(Shape::name("x"), Shape::neg(Shape::number(1.0))),
        ),
    ] {
        assert_bare_nonzero_numeric_rhs(statement, expected, &sentinel);
    }

    let double_neg_zero = Shape::sub(Shape::name("x"), Shape::neg(Shape::neg(Shape::number(0.0))));
    for (statement, expected) in [
        (
            "x = 0 * y;",
            Shape::sub(
                Shape::name("x"),
                Shape::binary(BinaryOp::Mul, Shape::number(0.0), Shape::name("y")),
            ),
        ),
        ("x = y;", Shape::sub(Shape::name("x"), Shape::name("y"))),
        (
            "x = zero(y);",
            Shape::sub(
                Shape::name("x"),
                Shape::Call("zero".to_owned(), vec![Shape::name("y")]),
            ),
        ),
        ("x = -(-0);", double_neg_zero.clone()),
    ] {
        assert_eq!(parse_shape_for_statement(statement), expected);
    }
    assert_eq!(format_statement("x = 0 * y;"), "x = 0 * y;");
    assert_eq!(format_statement("x = y;"), "x = y;");
    assert_eq!(format_statement("x = zero(y);"), "x = zero(y);");
    let canonical_double_neg = "x = --0;";
    assert_eq!(format_statement("x = -(-0);"), canonical_double_neg);
    let reparsed_double_neg = parse_shape_for_statement(canonical_double_neg);
    assert_eq!(reparsed_double_neg, double_neg_zero);
    assert_ne!(
        reparsed_double_neg, sentinel,
        "sentinel-lookahead mutant survived"
    );
    assert_eq!(format_statement(canonical_double_neg), canonical_double_neg);
    assert_eq!(format_statement("x = ((0));"), "x = (0);");
}

fn assert_bare_nonzero_numeric_rhs(statement: &str, expected: Shape, sentinel: &Shape) {
    let source = format!(
        "model m {{ field x: 1 = 1; parameter y: 1 = 0; relation r continuous {{ {statement} }} }}"
    );
    let document = parse("bare-nonzero.eqi", &source)
        .into_document()
        .unwrap_or_else(|diagnostics| {
            panic!("bare-nonzero RHS admission mutant survived: {diagnostics:#?}")
        });
    let actual = relation_shapes(&document, "r")
        .into_iter()
        .next()
        .expect("one bare-nonzero statement root");
    assert_eq!(actual, expected, "zero-equality-guard mutant survived");
    assert_ne!(
        &actual, sentinel,
        "finite nonzero Number was misclassified as the zero sentinel"
    );
    ModelDocument::compile("bare-nonzero.eqi", &source)
        .expect("bare finite nonzero numeric RHS must compile in the dimensionless setup");

    let canonical = format_statement(statement);
    assert_eq!(canonical, statement);
    let reparsed = parse_shape_for_statement(&canonical);
    assert_eq!(reparsed, expected);
    assert_ne!(&reparsed, sentinel);
    assert_eq!(format_statement(&canonical), canonical);
}

fn assert_zero_case(statement: &str, expected: Shape, formatted: &str) {
    let actual = parse_shape_for_statement(statement);
    assert_eq!(actual, expected);
    let canonical = format_statement(statement);
    assert_eq!(canonical, formatted);
    let reparsed = parse_shape_for_statement(&canonical);
    assert_eq!(reparsed, expected);
    assert_eq!(format_statement(&canonical), formatted);

    let mutants: &[&str] = if formatted == "x = (0);" {
        &["x = 0;", "x = ((0));", "x = 1e-400;", "x = (1e-400);"]
    } else {
        &["x = -0;", "x = (-(-0));", "x = (0);", "x = ((-0));"]
    };
    assert!(!mutants.contains(&canonical.as_str()));
}

fn assert_independent_side_precedence() {
    let source = "model m { field a: 1 = 1; field b: 1 = 2; field c: 1 = 3; field d: 1 = 4; relation r continuous { a - b = c; a = b - c; -a = -b; a - (b - c) = d; } }";
    let expected = vec![
        Shape::sub(
            Shape::sub(Shape::name("a"), Shape::name("b")),
            Shape::name("c"),
        ),
        Shape::sub(
            Shape::name("a"),
            Shape::sub(Shape::name("b"), Shape::name("c")),
        ),
        Shape::sub(Shape::neg(Shape::name("a")), Shape::neg(Shape::name("b"))),
        Shape::sub(
            Shape::sub(
                Shape::name("a"),
                Shape::sub(Shape::name("b"), Shape::name("c")),
            ),
            Shape::name("d"),
        ),
    ];
    let document = parsed("precedence.eqi", source);
    assert_eq!(relation_shapes(&document, "r"), expected);
    let statements = formatted_statements(source);
    assert_eq!(
        statements,
        ["a - b = c;", "a = b - c;", "-a = -b;", "a - (b - c) = d;"]
    );
    let reparsed = parsed("precedence-formatted.eqi", &format(&document));
    assert_eq!(relation_shapes(&reparsed, "r"), expected);
    assert_eq!(format(&reparsed), format(&document));

    let mutants = [
        ["a = b - c;", "a - b - c = 0;", "c = a - b;"],
        ["a - b = c;", "a - b - c = 0;", "a = (b - c);"],
        ["a = b;", "a = -b;", "-a + b = 0;"],
        ["a - b - c = d;", "a = b - c - d;", "(a - (b - c)) = d;"],
    ];
    for (actual, wrong) in statements.iter().zip(mutants) {
        for mutant in wrong {
            assert_ne!(actual, mutant, "precedence/side-context mutant survived");
        }
    }
}

fn assert_parser_owned_ranges() {
    const SOURCE: &str = "model m { field lhs: 1 = 1; field rhs: 1 = 2; relation r continuous { lhs = (rhs); lhs = rhs; } }";
    let document = parsed("ranges.eqi", SOURCE);
    let relation_decl = relation(&document, "r");
    assert_eq!(
        (relation_decl.range().start(), relation_decl.range().end()),
        (46, 95)
    );
    assert_eq!(relation_decl.residuals().len(), 2);

    let first = &relation_decl.residuals()[0];
    let ExprKind::Binary {
        op: BinaryOp::Sub,
        left,
        right,
    } = first.kind()
    else {
        panic!("parenthesized natural RHS must synthesize Sub");
    };
    assert_eq!((first.range().start(), first.range().end()), (70, 81));
    assert_eq!((left.range().start(), left.range().end()), (70, 73));
    assert_eq!((right.range().start(), right.range().end()), (76, 81));

    let second = &relation_decl.residuals()[1];
    let ExprKind::Binary {
        op: BinaryOp::Sub,
        left,
        right,
    } = second.kind()
    else {
        panic!("second natural equality must synthesize Sub");
    };
    assert_eq!((second.range().start(), second.range().end()), (83, 92));
    assert_eq!((left.range().start(), left.range().end()), (83, 86));
    assert_eq!((right.range().start(), right.range().end()), (89, 92));

    for mutant in [(70, 73), (70, 80), (70, 82), (70, 92), (70, 95)] {
        assert_ne!((first.range().start(), first.range().end()), mutant);
    }

    let formatted = format(&document);
    let reparsed = parsed("formatted-ranges.eqi", &formatted);
    let reparsed_relation = relation(&reparsed, "r");
    assert_eq!(
        relation_shapes(&document, "r"),
        relation_shapes(&reparsed, "r")
    );
    assert_ne!(relation_decl.range(), reparsed_relation.range());
}

fn assert_fail_closed_order_and_baseline_diagnostics() {
    let cases = [
        (
            "lhs-intrinsic",
            "model m { field rhs: 1 = 1; relation r continuous { missing - rhs = 0; } }",
            "model m { field rhs: 1 = 1; relation r continuous { missing = rhs; } }",
            true,
        ),
        (
            "rhs-intrinsic",
            "model m { field lhs: 1 = 1; relation r continuous { lhs - missing = 0; } }",
            "model m { field lhs: 1 = 1; relation r continuous { lhs = missing; } }",
            true,
        ),
        (
            "dimension",
            "model m { field lhs: m = 1; field rhs: s = 1; relation r continuous { lhs - rhs = 0; } }",
            "model m { field lhs: m = 1; field rhs: s = 1; relation r continuous { lhs = rhs; } }",
            true,
        ),
        (
            "shape",
            "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field lhs on body as space: 1 shape [3]; field rhs on body as space: 1 shape spatial_vector; relation r continuous on body { lhs - rhs = 0; } }",
            "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field lhs on body as space: 1 shape [3]; field rhs on body as space: 1 shape spatial_vector; relation r continuous on body { lhs = rhs; } }",
            false,
        ),
        (
            "frame",
            "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field lhs on body as space: 1 shape [2]; field rhs on body as space: 1 shape spatial_vector; relation r continuous on body { lhs - rhs = 0; } }",
            "model m { domain body = box(0, 1, 0, 1); representation space = continuum; field lhs on body as space: 1 shape [2]; field rhs on body as space: 1 shape spatial_vector; relation r continuous on body { lhs = rhs; } }",
            false,
        ),
        (
            "nominal-support",
            "model m { domain a = box(0, 1); domain b = box(0, 1); representation space = continuum; field lhs on a as space: 1 = 1; field rhs on b as space: 1 = 1; relation r continuous on a { lhs - rhs = 0; } }",
            "model m { domain a = box(0, 1); domain b = box(0, 1); representation space = continuum; field lhs on a as space: 1 = 1; field rhs on b as space: 1 = 1; relation r continuous on a { lhs = rhs; } }",
            false,
        ),
        (
            "root-support",
            "model m { domain a = box(0, 1); domain b = box(0, 1); representation space = continuum; field lhs on a as space: 1 = 1; field rhs on a as space: 1 = 1; relation r continuous on b { lhs - rhs = 0; } }",
            "model m { domain a = box(0, 1); domain b = box(0, 1); representation space = continuum; field lhs on a as space: 1 = 1; field rhs on a as space: 1 = 1; relation r continuous on b { lhs = rhs; } }",
            false,
        ),
    ];
    for (name, explicit, natural, has_span) in cases {
        let explicit_diagnostics = rejected(name, explicit);
        let natural_diagnostics = rejected(name, natural);
        let explicit_projection = project_diagnostics(&explicit_diagnostics);
        let natural_projection = project_diagnostics(&natural_diagnostics);
        assert_eq!(
            natural_projection, explicit_projection,
            "{name} route changed"
        );
        assert_eq!(natural_projection.len(), 1, "{name} must fail closed once");
        assert_eq!(
            natural_projection[0].span.is_some(),
            has_span,
            "{name} span owner"
        );
        if !has_span {
            assert_ne!(natural_projection[0].span, equation_span(natural));
        }
    }

    let both_missing_explicit = "model m { field lhs: 1 = 1; field rhs: 1 = 2; relation r continuous { missing_left - missing_right = 0; } }";
    let both_missing_natural = "model m { field lhs: 1 = 1; field rhs: 1 = 2; relation r continuous { missing_left = missing_right; } }";
    let explicit = project_diagnostics(&rejected("both-missing", both_missing_explicit));
    let natural = project_diagnostics(&rejected("both-missing", both_missing_natural));
    assert_eq!(natural, explicit);
    assert!(natural[0].message_class.contains("missing_left"));
    assert!(!natural[0].message_class.contains("missing_right"));

    let ordered_explicit = "model m { field lhs: 1 = 1; field rhs: 1 = 2; relation r continuous { missing_first - rhs = 0; lhs - missing_second = 0; } }";
    let ordered_natural = "model m { field lhs: 1 = 1; field rhs: 1 = 2; relation r continuous { missing_first = rhs; lhs = missing_second; } }";
    let explicit = project_diagnostics(&rejected("ordered-errors.eqi", ordered_explicit));
    let natural = project_diagnostics(&rejected("ordered-errors.eqi", ordered_natural));
    assert_eq!(natural, explicit);
    assert_eq!(
        natural.len(),
        1,
        "current route stops at the first bad statement"
    );
    assert!(natural[0].message_class.contains("missing_first"));
    assert!(!natural[0].message_class.contains("missing_second"));

    for malformed in [
        "model m { field lhs: 1 = 1; relation r continuous { lhs = ; } }",
        "model m { field lhs: 1 = 1; relation r continuous { = lhs; } }",
        "model m { field lhs: 1 = 1; relation r continuous { lhs = lhs = lhs; } }",
        "model m { field lhs: 1 = 1; relation r continuous { lhs = 1e999; } }",
        "model m { field lhs: 1 = 1; relation r continuous { lhs = lhs } }",
    ] {
        let diagnostics = rejected("malformed.eqi", malformed);
        assert!(!diagnostics.is_empty());
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.source_span().is_some())
        );
    }
}

fn assert_exact_locked_package_path(explicit_reference: &ModelDocument) {
    let release = prepare_package_release_v1(package_sources(NATURAL), &[])
        .expect("natural source must prepare one exact release");
    let release_bytes = release.canonical_json().unwrap();
    assert_eq!(
        PackageReleaseV1::from_json(&release_bytes).unwrap(),
        release
    );

    let resolution = ResolutionRecordV1::from_exact_releases(&release, &[])
        .expect("derive exact one-package lock");
    let resolution_bytes = resolution.canonical_json().unwrap();
    assert_eq!(
        ResolutionRecordV1::from_json(&resolution_bytes).unwrap(),
        resolution
    );
    assert_eq!(resolution.root(), &release.package_identity().unwrap());
    assert_eq!(resolution.nodes().len(), 1);
    assert!(resolution.edges().is_empty());

    let mut store = InMemoryPackageStore::default();
    let stored = store.insert(&release).expect("store exact natural release");
    assert_eq!(stored, release.source_digest().unwrap());
    let packaged = PackagedModelDocument::compile_locked(&store, &resolution, "NaturalEquation")
        .expect("compile natural source through exact locked package path");
    packaged
        .compilation()
        .validate_against(&resolution)
        .unwrap();
    let compilation_bytes = packaged.compilation().canonical_json().unwrap();
    assert_eq!(
        PackageCompilationRecordV1::from_json(&compilation_bytes).unwrap(),
        *packaged.compilation()
    );
    assert!(
        packaged
            .model()
            .structurally_equivalent(explicit_reference)
            .unwrap()
    );
    assert_eq!(
        packaged.model().structural_fingerprint().unwrap(),
        explicit_reference.structural_fingerprint().unwrap()
    );

    // Exact lineages remain independent; no cross-package identity equality
    // is used as evidence for equation meaning.
    assert_ne!(
        packaged.model().artifact_reference().unwrap(),
        explicit_reference.artifact_reference().unwrap()
    );
    assert_ne!(
        packaged.model().canonical_json().unwrap(),
        explicit_reference.canonical_json().unwrap()
    );
}

fn assert_native_explicit_residual(natural: &ModelDocument) {
    let lhs = DraftField::new("lhs", DimExponents::DIMENSIONLESS, 3.0);
    let rhs = DraftParameter::new("rhs", DimExponents::DIMENSIONLESS, 2.0);
    let relation = DraftRelation::continuous("balance", [lhs.expression() - rhs.expression()]);
    let draft = ModelDraft::new("NaturalEquation", [lhs.into(), rhs.into(), relation.into()])
        .expect("native explicit residual draft");
    let native = ModelDocument::define(&draft).expect("native explicit residual compiles");
    assert!(native.structurally_equivalent(natural).unwrap());
    assert_eq!(
        native.structural_fingerprint().unwrap(),
        natural.structural_fingerprint().unwrap()
    );
    assert_ne!(native.aliases()["lhs"], natural.aliases()["lhs"]);
    assert_ne!(
        native.artifact_reference().unwrap(),
        natural.artifact_reference().unwrap()
    );
    assert_ne!(
        native.canonical_json().unwrap(),
        natural.canonical_json().unwrap()
    );
}

fn package_sources(source: &str) -> AuthorPackageSourcesV1 {
    let path = NormalizedRelativePath::parse("src/main.eqi").expect("fixture path");
    let manifest = AuthorManifestV1::new(
        QualifiedName::parse("org.eqiora.verify.NaturalEquation").expect("package name"),
        ExactVersion::parse("0.1.0").expect("package version"),
        Vec::new(),
        vec![BundleEntryV1::new(path.clone(), BundleRoleV1::ModelSource)],
    )
    .expect("closed manifest");
    AuthorPackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            path,
            BundleRoleV1::ModelSource,
            source.as_bytes().to_vec(),
        )],
    )
    .expect("bounded source bundle")
}

fn parse_shape_for_statement(statement: &str) -> Shape {
    let source = format!(
        "model m {{ field x: 1 = 1; parameter y: 1 = 0; relation r continuous {{ {statement} }} }}"
    );
    relation_shapes(&parsed("statement.eqi", &source), "r")
        .into_iter()
        .next()
        .expect("one statement root")
}

fn format_statement(statement: &str) -> String {
    let source = format!(
        "model m {{ field x: 1 = 1; parameter y: 1 = 0; relation r continuous {{ {statement} }} }}"
    );
    formatted_statements(&source)
        .into_iter()
        .next()
        .expect("one formatted statement")
}

fn formatted_statements(source: &str) -> Vec<String> {
    let formatted = format(&parsed("format.eqi", source));
    let mut in_relation = false;
    let mut statements = Vec::new();
    for line in formatted.lines() {
        let line = line.trim();
        if line.starts_with("relation ") && line.ends_with('{') {
            in_relation = true;
        } else if in_relation && line == "}" {
            break;
        } else if in_relation && line.ends_with(';') {
            statements.push(line.to_owned());
        }
    }
    statements
}

fn parsed(filename: &str, source: &str) -> eqiora::language::Document {
    parse(filename, source)
        .into_document()
        .unwrap_or_else(|diagnostics| panic!("{filename} did not parse: {diagnostics:#?}"))
}

fn relation<'a>(
    document: &'a eqiora::language::Document,
    name: &str,
) -> &'a eqiora::language::RelationDecl {
    document
        .models()
        .iter()
        .flat_map(|model| model.items())
        .find_map(|item| match item {
            Item::Relation(relation) if relation.name() == name => Some(relation),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing Relation `{name}`"))
}

fn relation_shapes(document: &eqiora::language::Document, name: &str) -> Vec<Shape> {
    relation(document, name)
        .residuals()
        .iter()
        .map(shape)
        .collect()
}

fn shape(expression: &Expr) -> Shape {
    match expression.kind() {
        ExprKind::Number(value) => Shape::number(*value),
        ExprKind::Name(name) => Shape::Name(name.clone()),
        ExprKind::Path(path) => Shape::Path(path.as_str().to_owned()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => Shape::neg(shape(value)),
        ExprKind::Binary { op, left, right } => Shape::binary(*op, shape(left), shape(right)),
        ExprKind::Call { callee, arguments } => Shape::Call(
            callee.as_str().to_owned(),
            arguments.iter().map(shape).collect(),
        ),
        ExprKind::BoundaryPortSelection { .. } => {
            panic!("cell A evidence does not use boundary selectors")
        }
        _ => panic!("cell A evidence encountered an unknown expression form"),
    }
}

fn rejected(filename: &str, source: &str) -> Vec<eqiora::Diagnostic> {
    ModelDocument::compile(filename, source)
        .expect_err("rejecting source must not return an accepted Model")
}

fn project_diagnostics(diagnostics: &[eqiora::Diagnostic]) -> Vec<DiagnosticProjection> {
    diagnostics
        .iter()
        .map(|diagnostic| DiagnosticProjection {
            code: diagnostic.code().0,
            message_class: diagnostic_message_class(diagnostic.message()),
            graph_path: diagnostic.graph_path().map(|path| {
                path.segments()
                    .iter()
                    .map(|segment| {
                        if segment.starts_with("Relation:") {
                            "Relation:<fresh-occurrence>".to_owned()
                        } else {
                            segment.clone()
                        }
                    })
                    .collect()
            }),
            span: diagnostic.source_span().map(|span| (span.start, span.end)),
        })
        .collect()
}

fn diagnostic_message_class(message: &str) -> String {
    let mut normalized = String::new();
    let mut rest = message;
    while let Some(start) = rest.find("Ulid(") {
        normalized.push_str(&rest[..start]);
        let dynamic = &rest[start + "Ulid(".len()..];
        let end = dynamic
            .find(')')
            .expect("diagnostic ULID debug form must close");
        normalized.push_str("Ulid(<fresh-occurrence>)");
        rest = &dynamic[end + 1..];
    }
    normalized.push_str(rest);
    normalized
}

fn equation_span(source: &str) -> Option<(u32, u32)> {
    let marker = "lhs = rhs";
    source
        .find(marker)
        .map(|start| (start as u32, (start + marker.len()) as u32))
}
