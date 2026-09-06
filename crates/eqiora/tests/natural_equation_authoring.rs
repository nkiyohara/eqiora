use eqiora::api::ModelDocument;
use eqiora::language::{
    BinaryOp, Document, DraftField, DraftRelation, Expr, ExprKind, Item, ModelDraft, RelationDecl,
    UnaryOp, format, parse,
};
use eqiora::package::{
    BundleEntryV1, BundleRoleV1, ExactVersion, InMemoryPackageStore, NormalizedRelativePath,
    PackageManifestV1, PackageSourcesV1, PackagedModelDocument, QualifiedName, ResolutionRecordV1,
    SourceFileV1, prepare_package_release_v1,
};
use eqiora::{Diagnostic, DimExponents};

const NATURAL: &str =
    include_str!("../../../verify/language/natural-equation-authoring/models/natural.eqi");
const EXPLICIT: &str = include_str!(
    "../../../verify/language/natural-equation-authoring/models/explicit-residual.eqi"
);
const MALFORMED: &str = "model d { field x: 1 = 1; field y: 1 = 2; field z: 1 = 3; relation r continuous { x = y = z; } }";
const DIMENSIONFUL_POSITIVE_ZERO: &str = "model dimensionful_sentinel_probe {\n  field force: m = 1;\n  relation balance continuous {\n    force = 0;\n  }\n}\n";
const DIMENSIONFUL_NEGATIVE_ZERO: &str = "model dimensionful_sentinel_probe {\n  field force: m = 1;\n  relation balance continuous {\n    force = -0;\n  }\n}\n";
const DIMENSIONFUL_UNDERFLOW_ZERO: &str = "model dimensionful_sentinel_probe {\n  field force: m = 1;\n  relation balance continuous {\n    force = 1e-324;\n  }\n}\n";

#[derive(Clone, Debug, PartialEq, Eq)]
enum Tree {
    Number(u64),
    Name(String),
    Neg(Box<Self>),
    Add(Box<Self>, Box<Self>),
    Sub(Box<Self>, Box<Self>),
    Mul(Box<Self>, Box<Self>),
    Div(Box<Self>, Box<Self>),
    Pow(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RangedTree {
    tree: Tree,
    range: (u32, u32),
    children: Vec<Self>,
}

#[derive(Default)]
struct Accounting {
    public_operations: usize,
    formatter_bytes: usize,
    projected_diagnostics: usize,
}

impl Accounting {
    fn operation(&mut self) {
        self.public_operations = self
            .public_operations
            .checked_add(1)
            .expect("public-operation accounting overflow");
        assert!(
            self.public_operations <= 262,
            "public-operation cap exceeded"
        );
    }

    fn formatted(&mut self, bytes: usize) {
        assert!(bytes <= 1024, "per-document formatter cap exceeded");
        self.formatter_bytes = self
            .formatter_bytes
            .checked_add(bytes)
            .expect("formatter-byte accounting overflow");
        assert!(
            self.formatter_bytes <= 8192,
            "aggregate formatter cap exceeded"
        );
    }

    fn diagnostics(&mut self, count: usize) {
        assert!(count <= 2, "per-source diagnostic cap exceeded");
        self.projected_diagnostics = self
            .projected_diagnostics
            .checked_add(count)
            .expect("diagnostic accounting overflow");
        assert!(
            self.projected_diagnostics <= 40,
            "aggregate diagnostic cap exceeded"
        );
    }
}

fn n(name: &str) -> Tree {
    Tree::Name(name.to_owned())
}

fn z() -> Tree {
    Tree::Number(0.0_f64.to_bits())
}

fn neg(value: Tree) -> Tree {
    Tree::Neg(Box::new(value))
}

fn add(left: Tree, right: Tree) -> Tree {
    Tree::Add(Box::new(left), Box::new(right))
}

fn sub(left: Tree, right: Tree) -> Tree {
    Tree::Sub(Box::new(left), Box::new(right))
}

fn mul(left: Tree, right: Tree) -> Tree {
    Tree::Mul(Box::new(left), Box::new(right))
}

fn projected_tree(expression: &Expr) -> Tree {
    match expression.kind() {
        ExprKind::Number(value) => Tree::Number(value.to_bits()),
        ExprKind::Name(name) => Tree::Name(name.clone()),
        ExprKind::Unary {
            op: UnaryOp::Neg,
            value,
        } => neg(projected_tree(value)),
        ExprKind::Binary { op, left, right } => {
            let left = projected_tree(left);
            let right = projected_tree(right);
            match op {
                BinaryOp::Add => add(left, right),
                BinaryOp::Sub => sub(left, right),
                BinaryOp::Mul => mul(left, right),
                BinaryOp::Div => Tree::Div(Box::new(left), Box::new(right)),
                BinaryOp::Pow => Tree::Pow(Box::new(left), Box::new(right)),
            }
        }
        other => panic!("oracle corpus reached an unadmitted expression: {other:?}"),
    }
}

fn projected_ranged_tree(expression: &Expr) -> RangedTree {
    let children = match expression.kind() {
        ExprKind::Unary { value, .. } => vec![projected_ranged_tree(value)],
        ExprKind::Binary { left, right, .. } => {
            vec![projected_ranged_tree(left), projected_ranged_tree(right)]
        }
        ExprKind::Number(_) | ExprKind::Name(_) => vec![],
        other => panic!("oracle corpus reached an unadmitted ranged expression: {other:?}"),
    };
    let range = expression.range();
    RangedTree {
        tree: projected_tree(expression),
        range: (range.start(), range.end()),
        children,
    }
}

fn ranged_leaf(tree: Tree, start: u32, end: u32) -> RangedTree {
    RangedTree {
        tree,
        range: (start, end),
        children: vec![],
    }
}

fn ranged_unary(tree: Tree, start: u32, end: u32, value: RangedTree) -> RangedTree {
    RangedTree {
        tree,
        range: (start, end),
        children: vec![value],
    }
}

fn ranged_binary(
    tree: Tree,
    start: u32,
    end: u32,
    left: RangedTree,
    right: RangedTree,
) -> RangedTree {
    RangedTree {
        tree,
        range: (start, end),
        children: vec![left, right],
    }
}

fn only_model(document: &Document) -> &eqiora::language::ModelDecl {
    let [model] = document.models() else {
        panic!("expected exactly one Model declaration")
    };
    model
}

fn only_relation(document: &Document) -> &RelationDecl {
    let relations = only_model(document)
        .items()
        .iter()
        .filter_map(|item| match item {
            Item::Relation(relation) => Some(relation),
            _ => None,
        })
        .collect::<Vec<_>>();
    let [relation] = relations.as_slice() else {
        panic!("expected exactly one Relation declaration")
    };
    relation
}

fn parse_document(file: &str, source: &str, accounting: &mut Accounting) -> Document {
    accounting.operation();
    parse(file, source)
        .into_document()
        .unwrap_or_else(|diagnostics| panic!("positive parse rejected: {diagnostics:?}"))
}

fn compile_document(filename: &str, source: &str, accounting: &mut Accounting) -> ModelDocument {
    accounting.operation();
    ModelDocument::compile(filename, source)
        .unwrap_or_else(|diagnostics| panic!("positive compilation rejected: {diagnostics:?}"))
}

fn format_document(document: &Document, accounting: &mut Accounting) -> String {
    accounting.operation();
    let formatted = format(document);
    accounting.formatted(formatted.len());
    formatted
}

fn roots(document: &Document) -> Vec<Tree> {
    only_relation(document)
        .residuals()
        .iter()
        .map(projected_tree)
        .collect()
}

fn ranged_roots(document: &Document) -> Vec<RangedTree> {
    only_relation(document)
        .residuals()
        .iter()
        .map(projected_ranged_tree)
        .collect()
}

fn statement_source(statement: &str) -> String {
    format!(
        "model statement_probe {{\n  field x: 1 = 1;\n  field y: 1 = 2;\n  field z: 1 = 3;\n  relation r continuous {{\n    {statement}\n    x = y;\n  }}\n}}\n"
    )
}

#[derive(Clone)]
struct StatementSpec {
    input: &'static str,
    first_root: Tree,
    golden: &'static str,
    relation_range: (u32, u32),
    root0_range: (u32, u32),
    root1_range: (u32, u32),
    exact_ranged_root: Option<RangedTree>,
}

fn statement_specs() -> Vec<StatementSpec> {
    vec![
        StatementSpec {
            input: "x = 0;",
            first_root: n("x"),
            golden: "x = 0;",
            relation_range: (80, 129),
            root0_range: (108, 109),
            root1_range: (119, 124),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = -0;",
            first_root: n("x"),
            golden: "x = 0;",
            relation_range: (80, 130),
            root0_range: (108, 109),
            root1_range: (120, 125),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = 1e-324;",
            first_root: n("x"),
            golden: "x = 0;",
            relation_range: (80, 134),
            root0_range: (108, 109),
            root1_range: (124, 129),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = (0);",
            first_root: sub(n("x"), z()),
            golden: "x = (0);",
            relation_range: (80, 131),
            root0_range: (108, 115),
            root1_range: (121, 126),
            exact_ranged_root: Some(ranged_binary(
                sub(n("x"), z()),
                108,
                115,
                ranged_leaf(n("x"), 108, 109),
                ranged_leaf(z(), 112, 115),
            )),
        },
        StatementSpec {
            input: "x = (-0);",
            first_root: sub(n("x"), neg(z())),
            golden: "x = (-0);",
            relation_range: (80, 132),
            root0_range: (108, 116),
            root1_range: (122, 127),
            exact_ranged_root: Some(ranged_binary(
                sub(n("x"), neg(z())),
                108,
                116,
                ranged_leaf(n("x"), 108, 109),
                ranged_unary(neg(z()), 112, 116, ranged_leaf(z(), 114, 115)),
            )),
        },
        StatementSpec {
            input: "x = (1e-324);",
            first_root: sub(n("x"), z()),
            golden: "x = (0);",
            relation_range: (80, 136),
            root0_range: (108, 120),
            root1_range: (126, 131),
            exact_ranged_root: Some(ranged_binary(
                sub(n("x"), z()),
                108,
                120,
                ranged_leaf(n("x"), 108, 109),
                ranged_leaf(z(), 112, 120),
            )),
        },
        StatementSpec {
            input: "x - 0 = 0;",
            first_root: sub(n("x"), z()),
            golden: "x = (0);",
            relation_range: (80, 133),
            root0_range: (108, 113),
            root1_range: (123, 128),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x - (-0) = 0;",
            first_root: sub(n("x"), neg(z())),
            golden: "x = (-0);",
            relation_range: (80, 136),
            root0_range: (108, 116),
            root1_range: (126, 131),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x - 1e-324 = 0;",
            first_root: sub(n("x"), z()),
            golden: "x = (0);",
            relation_range: (80, 138),
            root0_range: (108, 118),
            root1_range: (128, 133),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = ((0));",
            first_root: sub(n("x"), z()),
            golden: "x = (0);",
            relation_range: (80, 133),
            root0_range: (108, 117),
            root1_range: (123, 128),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = 0 * y;",
            first_root: sub(n("x"), mul(z(), n("y"))),
            golden: "x = 0 * y;",
            relation_range: (80, 133),
            root0_range: (108, 117),
            root1_range: (123, 128),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x = -(-0);",
            first_root: sub(n("x"), neg(neg(z()))),
            golden: "x = --0;",
            relation_range: (80, 133),
            root0_range: (108, 117),
            root1_range: (123, 128),
            exact_ranged_root: None,
        },
        StatementSpec {
            input: "x - y = z;",
            first_root: sub(sub(n("x"), n("y")), n("z")),
            golden: "x - y = z;",
            relation_range: (80, 133),
            root0_range: (108, 117),
            root1_range: (123, 128),
            exact_ranged_root: Some(ranged_binary(
                sub(sub(n("x"), n("y")), n("z")),
                108,
                117,
                ranged_binary(
                    sub(n("x"), n("y")),
                    108,
                    113,
                    ranged_leaf(n("x"), 108, 109),
                    ranged_leaf(n("y"), 112, 113),
                ),
                ranged_leaf(n("z"), 116, 117),
            )),
        },
        StatementSpec {
            input: "x = y - z;",
            first_root: sub(n("x"), sub(n("y"), n("z"))),
            golden: "x = y - z;",
            relation_range: (80, 133),
            root0_range: (108, 117),
            root1_range: (123, 128),
            exact_ranged_root: Some(ranged_binary(
                sub(n("x"), sub(n("y"), n("z"))),
                108,
                117,
                ranged_leaf(n("x"), 108, 109),
                ranged_binary(
                    sub(n("y"), n("z")),
                    112,
                    117,
                    ranged_leaf(n("y"), 112, 113),
                    ranged_leaf(n("z"), 116, 117),
                ),
            )),
        },
        StatementSpec {
            input: "-x = -y;",
            first_root: sub(neg(n("x")), neg(n("y"))),
            golden: "-x = -y;",
            relation_range: (80, 131),
            root0_range: (108, 115),
            root1_range: (121, 126),
            exact_ranged_root: Some(ranged_binary(
                sub(neg(n("x")), neg(n("y"))),
                108,
                115,
                ranged_unary(neg(n("x")), 108, 110, ranged_leaf(n("x"), 109, 110)),
                ranged_unary(neg(n("y")), 113, 115, ranged_leaf(n("y"), 114, 115)),
            )),
        },
        StatementSpec {
            input: "x - (y - z) = x;",
            first_root: sub(sub(n("x"), sub(n("y"), n("z"))), n("x")),
            golden: "x - (y - z) = x;",
            relation_range: (80, 139),
            root0_range: (108, 123),
            root1_range: (129, 134),
            exact_ranged_root: Some(ranged_binary(
                sub(sub(n("x"), sub(n("y"), n("z"))), n("x")),
                108,
                123,
                ranged_binary(
                    sub(n("x"), sub(n("y"), n("z"))),
                    108,
                    119,
                    ranged_leaf(n("x"), 108, 109),
                    ranged_binary(
                        sub(n("y"), n("z")),
                        112,
                        119,
                        ranged_leaf(n("y"), 113, 114),
                        ranged_leaf(n("z"), 117, 118),
                    ),
                ),
                ranged_leaf(n("x"), 122, 123),
            )),
        },
    ]
}

#[derive(Clone, Copy)]
enum MessageRule {
    Exact(&'static str),
    Prefix(&'static str),
    RootSupport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum GraphClass {
    None,
    SemanticExpression2,
}

#[derive(Clone)]
struct DiagnosticSpec {
    class: &'static str,
    natural: &'static str,
    explicit: &'static str,
    code: &'static str,
    message: MessageRule,
    graph: GraphClass,
    span: Option<(&'static str, u32, u32)>,
}

fn diagnostic_specs() -> Vec<DiagnosticSpec> {
    vec![
        DiagnosticSpec {
            class: "missing_lhs",
            natural: "model d { field rhs: 1 = 1; relation r continuous { missing = rhs; } }",
            explicit: "model d { field rhs: 1 = 1; relation r continuous { missing - rhs = 0; } }",
            code: "EQ0603",
            message: MessageRule::Exact("unresolved expression symbol `missing`"),
            graph: GraphClass::None,
            span: Some(("diagnostic.eqi", 52, 59)),
        },
        DiagnosticSpec {
            class: "missing_rhs",
            natural: "model d { field lhs: 1 = 1; relation r continuous { lhs = missing; } }",
            explicit: "model d { field lhs: 1 = 1; relation r continuous { lhs - missing = 0; } }",
            code: "EQ0603",
            message: MessageRule::Exact("unresolved expression symbol `missing`"),
            graph: GraphClass::None,
            span: Some(("diagnostic.eqi", 58, 65)),
        },
        DiagnosticSpec {
            class: "missing_both",
            natural: "model d { relation r continuous { left_missing = right_missing; } }",
            explicit: "model d { relation r continuous { left_missing - right_missing = 0; } }",
            code: "EQ0603",
            message: MessageRule::Exact("unresolved expression symbol `left_missing`"),
            graph: GraphClass::None,
            span: Some(("diagnostic.eqi", 34, 46)),
        },
        DiagnosticSpec {
            class: "dimension",
            natural: "model d { field distance: m = 1; field elapsed: s = 1; relation r continuous { distance = elapsed; } }",
            explicit: "model d { field distance: m = 1; field elapsed: s = 1; relation r continuous { distance - elapsed = 0; } }",
            code: "EQ0603",
            message: MessageRule::Exact("addition/subtraction combines dimensions [L] and [T]"),
            graph: GraphClass::None,
            span: Some(("diagnostic.eqi", 79, 97)),
        },
        DiagnosticSpec {
            class: "shape",
            natural: "model d { domain body = box(0, 1, 0, 1); representation space = continuum; field scalar on body as space: 1 = 0; field vector on body as space: vector<1, 2>; relation r continuous on body { scalar = vector; } }",
            explicit: "model d { domain body = box(0, 1, 0, 1); representation space = continuum; field scalar on body as space: 1 = 0; field vector on body as space: vector<1, 2>; relation r continuous on body { scalar - vector = 0; } }",
            code: "EQ0304",
            message: MessageRule::Prefix("addition/subtraction combines incompatible types "),
            graph: GraphClass::SemanticExpression2,
            span: None,
        },
        DiagnosticSpec {
            class: "frame",
            natural: "model d { domain body = box(0, 1, 0, 1); representation space = continuum; field invariant on body as space: array<1, 2>; field spatial on body as space: vector<1, 2>; relation r continuous on body { invariant = spatial; } }",
            explicit: "model d { domain body = box(0, 1, 0, 1); representation space = continuum; field invariant on body as space: array<1, 2>; field spatial on body as space: vector<1, 2>; relation r continuous on body { invariant - spatial = 0; } }",
            code: "EQ0304",
            message: MessageRule::Prefix("addition/subtraction combines incompatible types "),
            graph: GraphClass::SemanticExpression2,
            span: None,
        },
        DiagnosticSpec {
            class: "nominal_support",
            natural: "model d { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field left on a as space: 1 = 0; field right on b as space: 1 = 0; relation r continuous on a { left = right; } }",
            explicit: "model d { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field left on a as space: 1 = 0; field right on b as space: 1 = 0; relation r continuous on a { left - right = 0; } }",
            code: "EQ0302",
            message: MessageRule::Prefix("expression combines incompatible supports "),
            graph: GraphClass::SemanticExpression2,
            span: None,
        },
        DiagnosticSpec {
            class: "root_support",
            natural: "model d { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field foreign on b as space: 1 = 0; relation r continuous on a { foreign = foreign; } }",
            explicit: "model d { domain a = box(0, 1); domain b = box(0, 2); representation space = continuum; field foreign on b as space: 1 = 0; relation r continuous on a { foreign - foreign = 0; } }",
            code: "EQ0302",
            message: MessageRule::RootSupport,
            graph: GraphClass::SemanticExpression2,
            span: None,
        },
    ]
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SpanObservation {
    file: String,
    start: u32,
    end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct DiagnosticObservation {
    class: String,
    code: String,
    graph: GraphClass,
    span: Option<SpanObservation>,
}

fn observe_diagnostic(
    diagnostic: &Diagnostic,
    spec: &DiagnosticSpec,
    accounting: &mut Accounting,
) -> DiagnosticObservation {
    accounting.operation();
    let code = diagnostic.code().to_string();
    assert_eq!(code, spec.code);

    accounting.operation();
    let message = diagnostic.message();
    match spec.message {
        MessageRule::Exact(expected) => assert_eq!(message, expected),
        MessageRule::Prefix(prefix) => assert!(message.starts_with(prefix)),
        MessageRule::RootSupport => {
            assert!(message.starts_with("residual support "));
            assert!(message.contains(" differs from Relation scope "));
        }
    }

    accounting.operation();
    let graph = match (spec.graph, diagnostic.graph_path()) {
        (GraphClass::None, None) => GraphClass::None,
        (GraphClass::SemanticExpression2, Some(path)) => {
            let path = path.to_string();
            assert!(path.starts_with("semantic.Relation.Relation:"));
            assert!(path.ends_with(".expression.2"));
            GraphClass::SemanticExpression2
        }
        (expected, actual) => panic!("graph-path class mismatch: {expected:?}, {actual:?}"),
    };

    accounting.operation();
    let span = diagnostic.source_span().map(|span| SpanObservation {
        file: span.file.clone(),
        start: span.start,
        end: span.end,
    });
    let expected_span = spec.span.map(|(file, start, end)| SpanObservation {
        file: file.to_owned(),
        start,
        end,
    });
    assert_eq!(span, expected_span);

    DiagnosticObservation {
        class: spec.class.to_owned(),
        code,
        graph,
        span,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AdditiveObservation {
    class: String,
    logical_name: String,
    field_name: String,
    dimension: Tree,
    initializer_bits: u64,
    relation_name: String,
    root: Tree,
    compiled: bool,
}

fn observe_dimensionful_sentinel(
    class: &str,
    logical_name: &str,
    source: &str,
    accounting: &mut Accounting,
) -> AdditiveObservation {
    let document = parse_document(logical_name, source, accounting);
    let model = only_model(&document);
    assert_eq!(model.name(), "dimensionful_sentinel_probe");
    assert_eq!(model.items().len(), 2);
    let field = match &model.items()[0] {
        Item::Field(field) => field,
        other => panic!("expected the Field first, found {other:?}"),
    };
    assert_eq!(field.name(), "force");
    assert_eq!(projected_tree(field.dimension()), n("m"));
    let ExprKind::Number(initializer) = field.initial().expect("initializer").kind() else {
        panic!("expected contextual numeric initializer");
    };
    assert_eq!(initializer.to_bits(), 1.0_f64.to_bits());
    let relation = match &model.items()[1] {
        Item::Relation(relation) => relation,
        other => panic!("expected the Relation second, found {other:?}"),
    };
    assert_eq!(relation.name(), "balance");
    assert_eq!(relation.residuals().len(), 1);
    let root = projected_tree(&relation.residuals()[0]);
    assert_eq!(root, n("force"));

    let _model = compile_document(logical_name, source, accounting);
    AdditiveObservation {
        class: class.to_owned(),
        logical_name: logical_name.to_owned(),
        field_name: field.name().to_owned(),
        dimension: projected_tree(field.dimension()),
        initializer_bits: initializer.to_bits(),
        relation_name: relation.name().to_owned(),
        root,
        compiled: true,
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct EqualityPolicy {
    structurally_equivalent: bool,
    fingerprint_equal: bool,
    exact_equalities: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ComparisonKind {
    StructuralStatic,
    ExactArtifact,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Observation {
    Tree(Tree),
    OptionalTree(Option<Tree>),
    Trees(Vec<Tree>),
    Text(String),
    Offset(u32),
    Span(Option<SpanObservation>),
    Boolean(bool),
    Policy(EqualityPolicy),
    Comparison(ComparisonKind),
    Additive(AdditiveObservation),
}

struct MutantRecord {
    family: &'static str,
    actual: Observation,
    accepted: Observation,
    mutant: Observation,
}

fn first_line_statement(formatted: &str, index: usize) -> String {
    formatted
        .lines()
        .nth(index)
        .unwrap_or_else(|| panic!("formatted statement line {index} is absent"))
        .trim()
        .to_owned()
}

fn binary_operands(tree: &Tree) -> Vec<Tree> {
    match tree {
        Tree::Sub(left, right)
        | Tree::Add(left, right)
        | Tree::Mul(left, right)
        | Tree::Div(left, right)
        | Tree::Pow(left, right) => vec![left.as_ref().clone(), right.as_ref().clone()],
        other => panic!("expected binary tree, found {other:?}"),
    }
}

fn binary_right(tree: &Tree) -> Option<Tree> {
    match tree {
        Tree::Sub(_, right)
        | Tree::Add(_, right)
        | Tree::Mul(_, right)
        | Tree::Div(_, right)
        | Tree::Pow(_, right) => Some(right.as_ref().clone()),
        _ => None,
    }
}

#[test]
fn revised_oracle_sequence() {
    let mut accounting = Accounting::default();
    let statement_specs = statement_specs();
    let diagnostic_specs = diagnostic_specs();
    assert_eq!(statement_specs.len(), 16);
    assert_eq!(diagnostic_specs.len(), 8);
    let expected_record_count = statement_specs
        .len()
        .checked_add(diagnostic_specs.len())
        .and_then(|count| count.checked_add(2))
        .and_then(|count| count.checked_add(3))
        .expect("private expected-record count overflow");
    assert!(
        expected_record_count <= 64,
        "private expected-record cap exceeded"
    );

    // Stage 2: ordinary nonzero public natural equality accepts before every denial.
    let natural_model = compile_document("natural.eqi", NATURAL, &mut accounting);

    // Stage 3: exact trees/ranges, explicit compile, structural meaning, and identity guards.
    let natural_document = parse_document("natural.eqi", NATURAL, &mut accounting);
    let explicit_document = parse_document("explicit-residual.eqi", EXPLICIT, &mut accounting);
    let positive_trees = vec![
        sub(n("a"), n("b")),
        sub(sub(n("a"), sub(n("b"), n("c"))), n("d")),
        sub(neg(n("a")), neg(n("b"))),
    ];
    assert_eq!(roots(&natural_document), positive_trees);
    assert_eq!(roots(&explicit_document), positive_trees);
    assert_eq!(only_relation(&natural_document).name(), "balance");
    assert_eq!(only_relation(&natural_document).range().start(), 106);
    assert_eq!(only_relation(&natural_document).range().end(), 184);
    assert_eq!(only_relation(&explicit_document).range().start(), 106);
    assert_eq!(only_relation(&explicit_document).range().end(), 196);

    let natural_ranged = ranged_roots(&natural_document);
    let expected_natural_ranged = vec![
        ranged_binary(
            sub(n("a"), n("b")),
            140,
            145,
            ranged_leaf(n("a"), 140, 141),
            ranged_leaf(n("b"), 144, 145),
        ),
        ranged_binary(
            sub(sub(n("a"), sub(n("b"), n("c"))), n("d")),
            151,
            166,
            ranged_binary(
                sub(n("a"), sub(n("b"), n("c"))),
                151,
                162,
                ranged_leaf(n("a"), 151, 152),
                ranged_binary(
                    sub(n("b"), n("c")),
                    155,
                    162,
                    ranged_leaf(n("b"), 156, 157),
                    ranged_leaf(n("c"), 160, 161),
                ),
            ),
            ranged_leaf(n("d"), 165, 166),
        ),
        ranged_binary(
            sub(neg(n("a")), neg(n("b"))),
            172,
            179,
            ranged_unary(neg(n("a")), 172, 174, ranged_leaf(n("a"), 173, 174)),
            ranged_unary(neg(n("b")), 177, 179, ranged_leaf(n("b"), 178, 179)),
        ),
    ];
    assert_eq!(natural_ranged, expected_natural_ranged);

    let expected_explicit_ranged = vec![
        ranged_binary(
            sub(n("a"), n("b")),
            140,
            145,
            ranged_leaf(n("a"), 140, 141),
            ranged_leaf(n("b"), 144, 145),
        ),
        ranged_binary(
            sub(sub(n("a"), sub(n("b"), n("c"))), n("d")),
            155,
            170,
            ranged_binary(
                sub(n("a"), sub(n("b"), n("c"))),
                155,
                166,
                ranged_leaf(n("a"), 155, 156),
                ranged_binary(
                    sub(n("b"), n("c")),
                    159,
                    166,
                    ranged_leaf(n("b"), 160, 161),
                    ranged_leaf(n("c"), 164, 165),
                ),
            ),
            ranged_leaf(n("d"), 169, 170),
        ),
        ranged_binary(
            sub(neg(n("a")), neg(n("b"))),
            180,
            187,
            ranged_unary(neg(n("a")), 180, 182, ranged_leaf(n("a"), 181, 182)),
            ranged_unary(neg(n("b")), 185, 187, ranged_leaf(n("b"), 186, 187)),
        ),
    ];
    assert_eq!(ranged_roots(&explicit_document), expected_explicit_ranged);

    let explicit_model = compile_document("explicit-residual.eqi", EXPLICIT, &mut accounting);
    accounting.operation();
    let structurally_equivalent = natural_model
        .structurally_equivalent(&explicit_model)
        .expect("public structural comparison");
    assert!(structurally_equivalent);
    accounting.operation();
    let natural_fingerprint = natural_model
        .structural_fingerprint()
        .expect("natural structural fingerprint")
        .to_string();
    accounting.operation();
    let explicit_fingerprint = explicit_model
        .structural_fingerprint()
        .expect("explicit structural fingerprint")
        .to_string();
    assert_eq!(natural_fingerprint, explicit_fingerprint);

    accounting.operation();
    let natural_reference = natural_model
        .artifact_reference()
        .expect("natural artifact reference");
    accounting.operation();
    let explicit_reference = explicit_model
        .artifact_reference()
        .expect("explicit artifact reference");
    assert_ne!(natural_reference.model(), explicit_reference.model());
    assert_ne!(natural_reference, explicit_reference);

    accounting.operation();
    let natural_canonical = natural_model
        .canonical_json()
        .expect("natural canonical Model bytes");
    accounting.operation();
    let explicit_canonical = explicit_model
        .canonical_json()
        .expect("explicit canonical Model bytes");
    assert!(natural_canonical.len() <= 256 * 1024);
    assert!(explicit_canonical.len() <= 256 * 1024);
    assert_ne!(natural_canonical, explicit_canonical);
    drop(natural_canonical);
    drop(explicit_canonical);

    accounting.operation();
    let natural_digest = natural_model.digest().expect("natural Model digest");
    accounting.operation();
    let explicit_digest = explicit_model.digest().expect("explicit Model digest");
    assert_ne!(natural_digest, explicit_digest);

    // Stage 4: both positives converge to the exact golden and are byte-idempotent.
    let natural_formatted = format_document(&natural_document, &mut accounting);
    assert_eq!(natural_formatted, NATURAL);
    let natural_reparsed = parse_document("natural.eqi", &natural_formatted, &mut accounting);
    assert_eq!(roots(&natural_reparsed), positive_trees);
    assert_eq!(ranged_roots(&natural_reparsed), expected_natural_ranged);
    assert_eq!(format_document(&natural_reparsed, &mut accounting), NATURAL);

    let explicit_formatted = format_document(&explicit_document, &mut accounting);
    assert_eq!(explicit_formatted, NATURAL);
    let explicit_reparsed = parse_document(
        "explicit-residual.eqi",
        &explicit_formatted,
        &mut accounting,
    );
    assert_eq!(roots(&explicit_reparsed), positive_trees);
    assert_eq!(ranged_roots(&explicit_reparsed), expected_natural_ranged);
    assert_eq!(
        format_document(&explicit_reparsed, &mut accounting),
        NATURAL
    );
    let positive_first_statement = first_line_statement(&natural_formatted, 6);

    // Stage 5: fixed sentinel, underflow, precedence, order, formatter, and range corpus.
    let mut statement_observations = Vec::with_capacity(statement_specs.len());
    for spec in &statement_specs {
        let source = statement_source(spec.input);
        let document = parse_document("statement.eqi", &source, &mut accounting);
        let relation = only_relation(&document);
        assert_eq!(relation.name(), "r");
        assert_eq!(
            (relation.range().start(), relation.range().end()),
            spec.relation_range
        );
        assert_eq!(relation.residuals().len(), 2);
        let first = projected_ranged_tree(&relation.residuals()[0]);
        let second = projected_ranged_tree(&relation.residuals()[1]);
        assert_eq!(first.tree, spec.first_root);
        assert_eq!(first.range, spec.root0_range);
        assert_eq!(second.tree, sub(n("x"), n("y")));
        assert_eq!(second.range, spec.root1_range);
        if let Some(expected) = &spec.exact_ranged_root {
            assert_eq!(&first, expected);
        }

        let golden = statement_source(spec.golden);
        let formatted = format_document(&document, &mut accounting);
        assert_eq!(formatted, golden);
        let formatted_statement = first_line_statement(&formatted, 5);
        let reparsed = parse_document("statement.eqi", &formatted, &mut accounting);
        assert_eq!(
            projected_tree(&only_relation(&reparsed).residuals()[0]),
            spec.first_root
        );
        assert_eq!(format_document(&reparsed, &mut accounting), golden);
        statement_observations.push((first, formatted_statement));
    }

    // Stage 6: each additive dimensionful positive passes before its falsifier.
    let additive_positive = observe_dimensionful_sentinel(
        "positive_zero",
        "dimensionful-positive-zero.eqi",
        DIMENSIONFUL_POSITIVE_ZERO,
        &mut accounting,
    );
    let additive_negative = observe_dimensionful_sentinel(
        "negative_zero",
        "dimensionful-negative-zero.eqi",
        DIMENSIONFUL_NEGATIVE_ZERO,
        &mut accounting,
    );
    let additive_underflow = observe_dimensionful_sentinel(
        "underflow_zero",
        "dimensionful-underflow-zero.eqi",
        DIMENSIONFUL_UNDERFLOW_ZERO,
        &mut accounting,
    );

    // Stage 7: malformed parser route, then every paired public diagnostic route.
    accounting.operation();
    let malformed = ModelDocument::compile("malformed.eqi", MALFORMED)
        .expect_err("malformed equality unexpectedly compiled");
    accounting.diagnostics(malformed.len());
    assert_eq!(malformed.len(), 2);
    accounting.operation();
    assert_eq!(malformed[0].code().to_string(), "EQ0602");
    accounting.operation();
    assert_eq!(malformed[0].message(), "expected `;` after residual");
    accounting.operation();
    let span0 = malformed[0].source_span().expect("first malformed span");
    assert_eq!(
        (&*span0.file, span0.start, span0.end),
        ("malformed.eqi", 88, 89)
    );
    accounting.operation();
    assert_eq!(malformed[1].code().to_string(), "EQ0602");
    accounting.operation();
    assert_eq!(
        malformed[1].message(),
        "expected `dimension`, `property`, `connector`, `component`, `pure operator`, or `model` declaration"
    );
    accounting.operation();
    let span1 = malformed[1].source_span().expect("second malformed span");
    assert_eq!(
        (&*span1.file, span1.start, span1.end),
        ("malformed.eqi", 95, 96)
    );

    let mut diagnostic_observations = Vec::with_capacity(diagnostic_specs.len());
    for spec in &diagnostic_specs {
        accounting.operation();
        let natural_diagnostics = ModelDocument::compile("diagnostic.eqi", spec.natural)
            .expect_err("natural diagnostic source unexpectedly compiled");
        accounting.diagnostics(natural_diagnostics.len());
        assert_eq!(natural_diagnostics.len(), 1);
        let natural_observation =
            observe_diagnostic(&natural_diagnostics[0], spec, &mut accounting);

        accounting.operation();
        let explicit_diagnostics = ModelDocument::compile("diagnostic.eqi", spec.explicit)
            .expect_err("explicit diagnostic source unexpectedly compiled");
        accounting.diagnostics(explicit_diagnostics.len());
        assert_eq!(explicit_diagnostics.len(), 1);
        let explicit_observation =
            observe_diagnostic(&explicit_diagnostics[0], spec, &mut accounting);
        assert_eq!(natural_observation, explicit_observation);
        diagnostic_observations.push(natural_observation);
    }
    assert_eq!(accounting.projected_diagnostics, 18);

    // Stage 8: the exact ordered 36-record one-field falsifier table.
    let mut mutants = Vec::with_capacity(36);
    mutants.push(MutantRecord {
        family: "operator",
        actual: Observation::Text("Sub".to_owned()),
        accepted: Observation::Text("Sub".to_owned()),
        mutant: Observation::Text("Mul".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "dropped rhs",
        actual: Observation::OptionalTree(binary_right(&positive_trees[0])),
        accepted: Observation::OptionalTree(Some(n("b"))),
        mutant: Observation::OptionalTree(None),
    });
    mutants.push(MutantRecord {
        family: "swapped operands",
        actual: Observation::Trees(binary_operands(&positive_trees[0])),
        accepted: Observation::Trees(vec![n("a"), n("b")]),
        mutant: Observation::Trees(vec![n("b"), n("a")]),
    });
    mutants.push(MutantRecord {
        family: "sign normalization",
        actual: Observation::OptionalTree(binary_right(&positive_trees[2])),
        accepted: Observation::OptionalTree(Some(neg(n("b")))),
        mutant: Observation::OptionalTree(Some(n("b"))),
    });
    let deepest = match &positive_trees[1] {
        Tree::Sub(left, _) => match left.as_ref() {
            Tree::Sub(_, right) => right.as_ref(),
            other => panic!("missing inner subtraction: {other:?}"),
        },
        other => panic!("missing second positive subtraction: {other:?}"),
    };
    mutants.push(MutantRecord {
        family: "operand order",
        actual: Observation::Trees(binary_operands(deepest)),
        accepted: Observation::Trees(vec![n("b"), n("c")]),
        mutant: Observation::Trees(vec![n("c"), n("b")]),
    });
    let mut reordered_roots = positive_trees.clone();
    reordered_roots.swap(0, 1);
    mutants.push(MutantRecord {
        family: "root order",
        actual: Observation::Trees(positive_trees.clone()),
        accepted: Observation::Trees(positive_trees.clone()),
        mutant: Observation::Trees(reordered_roots),
    });
    mutants.push(MutantRecord {
        family: "addition",
        actual: Observation::Text("Sub".to_owned()),
        accepted: Observation::Text("Sub".to_owned()),
        mutant: Observation::Text("Add".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "left precedence",
        actual: Observation::Tree(statement_observations[12].0.tree.clone()),
        accepted: Observation::Tree(sub(sub(n("x"), n("y")), n("z"))),
        mutant: Observation::Tree(sub(n("x"), sub(n("y"), n("z")))),
    });
    mutants.push(MutantRecord {
        family: "right precedence",
        actual: Observation::Tree(statement_observations[13].0.tree.clone()),
        accepted: Observation::Tree(sub(n("x"), sub(n("y"), n("z")))),
        mutant: Observation::Tree(sub(sub(n("x"), n("y")), n("z"))),
    });
    mutants.push(MutantRecord {
        family: "reassociation",
        actual: Observation::Tree(statement_observations[15].0.tree.clone()),
        accepted: Observation::Tree(sub(sub(n("x"), sub(n("y"), n("z"))), n("x"))),
        mutant: Observation::Tree(sub(sub(sub(n("x"), n("y")), n("z")), n("x"))),
    });
    mutants.push(MutantRecord {
        family: "side swap bytes",
        actual: Observation::Text(positive_first_statement.clone()),
        accepted: Observation::Text("a = b;".to_owned()),
        mutant: Observation::Text("b = a;".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "sentinel distinction",
        actual: Observation::Tree(statement_observations[0].0.tree.clone()),
        accepted: Observation::Tree(n("x")),
        mutant: Observation::Tree(sub(n("x"), z())),
    });
    mutants.push(MutantRecord {
        family: "zero escape omission",
        actual: Observation::Text(statement_observations[3].1.clone()),
        accepted: Observation::Text("x = (0);".to_owned()),
        mutant: Observation::Text("x = 0;".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "negative-zero escape omission",
        actual: Observation::Text(statement_observations[4].1.clone()),
        accepted: Observation::Text("x = (-0);".to_owned()),
        mutant: Observation::Text("x = -0;".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "Neg collapse",
        actual: Observation::Tree(statement_observations[4].0.tree.clone()),
        accepted: Observation::Tree(sub(n("x"), neg(z()))),
        mutant: Observation::Tree(sub(n("x"), z())),
    });
    mutants.push(MutantRecord {
        family: "underflow preservation",
        actual: Observation::Text(statement_observations[5].1.clone()),
        accepted: Observation::Text("x = (0);".to_owned()),
        mutant: Observation::Text("x = (1e-324);".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "extra grouping",
        actual: Observation::Text(statement_observations[9].1.clone()),
        accepted: Observation::Text("x = (0);".to_owned()),
        mutant: Observation::Text("x = ((0));".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "overbroad zero folding",
        actual: Observation::Tree(statement_observations[10].0.tree.clone()),
        accepted: Observation::Tree(sub(n("x"), mul(z(), n("y")))),
        mutant: Observation::Tree(n("x")),
    });
    mutants.push(MutantRecord {
        family: "overbroad grouping",
        actual: Observation::Text(statement_observations[10].1.clone()),
        accepted: Observation::Text("x = 0 * y;".to_owned()),
        mutant: Observation::Text("x = (0 * y);".to_owned()),
    });
    mutants.push(MutantRecord {
        family: "double-Neg collapse",
        actual: Observation::Tree(statement_observations[11].0.tree.clone()),
        accepted: Observation::Tree(sub(n("x"), neg(neg(z())))),
        mutant: Observation::Tree(sub(n("x"), z())),
    });
    mutants.push(MutantRecord {
        family: "root-start drift",
        actual: Observation::Offset(natural_ranged[0].range.0),
        accepted: Observation::Offset(140),
        mutant: Observation::Offset(141),
    });
    mutants.push(MutantRecord {
        family: "lhs-only root",
        actual: Observation::Offset(natural_ranged[0].range.1),
        accepted: Observation::Offset(145),
        mutant: Observation::Offset(141),
    });
    mutants.push(MutantRecord {
        family: "excluded RHS grouping",
        actual: Observation::Offset(statement_observations[3].0.range.1),
        accepted: Observation::Offset(115),
        mutant: Observation::Offset(114),
    });
    mutants.push(MutantRecord {
        family: "semicolon inclusion",
        actual: Observation::Offset(statement_observations[3].0.range.1),
        accepted: Observation::Offset(115),
        mutant: Observation::Offset(116),
    });
    mutants.push(MutantRecord {
        family: "next-statement drift",
        actual: Observation::Offset(statement_observations[3].0.range.1),
        accepted: Observation::Offset(115),
        mutant: Observation::Offset(121),
    });
    mutants.push(MutantRecord {
        family: "stale formatted offset",
        actual: Observation::Offset(expected_natural_ranged[1].range.0),
        accepted: Observation::Offset(151),
        mutant: Observation::Offset(155),
    });
    mutants.push(MutantRecord {
        family: "optional-span manufacture",
        actual: Observation::Span(diagnostic_observations[4].span.clone()),
        accepted: Observation::Span(None),
        mutant: Observation::Span(Some(SpanObservation {
            file: "diagnostic.eqi".to_owned(),
            start: 205,
            end: 220,
        })),
    });
    mutants.push(MutantRecord {
        family: "optional-span erasure",
        actual: Observation::Span(diagnostic_observations[3].span.clone()),
        accepted: Observation::Span(Some(SpanObservation {
            file: "diagnostic.eqi".to_owned(),
            start: 79,
            end: 97,
        })),
        mutant: Observation::Span(None),
    });
    let equality_policy = EqualityPolicy {
        structurally_equivalent: true,
        fingerprint_equal: true,
        exact_equalities: vec![],
    };
    mutants.push(MutantRecord {
        family: "identity overclaim",
        actual: Observation::Policy(equality_policy.clone()),
        accepted: Observation::Policy(equality_policy),
        mutant: Observation::Policy(EqualityPolicy {
            structurally_equivalent: true,
            fingerprint_equal: true,
            exact_equalities: vec![
                "canonical_bytes".to_owned(),
                "digest".to_owned(),
                "artifact_reference".to_owned(),
            ],
        }),
    });
    mutants.push(MutantRecord {
        family: "structural denial",
        actual: Observation::Boolean(structurally_equivalent),
        accepted: Observation::Boolean(true),
        mutant: Observation::Boolean(false),
    });
    mutants.push(MutantRecord {
        family: "package overclaim",
        actual: Observation::Comparison(ComparisonKind::StructuralStatic),
        accepted: Observation::Comparison(ComparisonKind::StructuralStatic),
        mutant: Observation::Comparison(ComparisonKind::ExactArtifact),
    });
    mutants.push(MutantRecord {
        family: "native overclaim",
        actual: Observation::Comparison(ComparisonKind::StructuralStatic),
        accepted: Observation::Comparison(ComparisonKind::StructuralStatic),
        mutant: Observation::Comparison(ComparisonKind::ExactArtifact),
    });
    let mut additive_positive_mutant = additive_positive.clone();
    additive_positive_mutant.root = sub(n("force"), z());
    mutants.push(MutantRecord {
        family: "dimensionful positive-zero sentinel loss",
        actual: Observation::Additive(additive_positive.clone()),
        accepted: Observation::Additive(additive_positive.clone()),
        mutant: Observation::Additive(additive_positive_mutant),
    });
    let mut additive_negative_mutant = additive_negative.clone();
    additive_negative_mutant.root = sub(n("force"), neg(z()));
    mutants.push(MutantRecord {
        family: "dimensionful negative-zero sentinel loss",
        actual: Observation::Additive(additive_negative.clone()),
        accepted: Observation::Additive(additive_negative.clone()),
        mutant: Observation::Additive(additive_negative_mutant),
    });
    let mut additive_underflow_mutant = additive_underflow.clone();
    additive_underflow_mutant.root = sub(n("force"), z());
    mutants.push(MutantRecord {
        family: "dimensionful underflow-zero sentinel loss",
        actual: Observation::Additive(additive_underflow.clone()),
        accepted: Observation::Additive(additive_underflow),
        mutant: Observation::Additive(additive_underflow_mutant),
    });
    assert_eq!(mutants.len(), 35);
    for mutant in &mutants {
        assert_eq!(
            mutant.actual, mutant.accepted,
            "passing observation rejected for {}",
            mutant.family
        );
        assert_ne!(
            mutant.actual, mutant.mutant,
            "one-field mutant survived for {}",
            mutant.family
        );
    }

    // Stage 9: exact locked-package path, compared only for structural meaning.
    let source_path =
        NormalizedRelativePath::parse("src/models/natural.eqi").expect("package source path");
    let manifest = PackageManifestV1::new(
        "models.natural",
        QualifiedName::parse("org.eqiora.oracle.NaturalEquation").expect("package name"),
        ExactVersion::parse("1.0.0").expect("package version"),
        vec![],
        vec![BundleEntryV1::new(
            source_path.clone(),
            BundleRoleV1::ModelSource,
        )],
    )
    .expect("package manifest");
    accounting.operation();
    let sources = PackageSourcesV1::new(
        manifest,
        vec![SourceFileV1::new(
            source_path,
            BundleRoleV1::ModelSource,
            NATURAL.as_bytes().to_vec(),
        )],
    )
    .expect("author package sources");
    accounting.operation();
    let release = prepare_package_release_v1(sources, &[]).expect("package preparation");
    let mut store = InMemoryPackageStore::default();
    accounting.operation();
    store.insert(&release).expect("package store insertion");
    accounting.operation();
    let resolution =
        ResolutionRecordV1::from_exact_releases(&release, &[]).expect("exact resolution");
    accounting.operation();
    let packaged =
        PackagedModelDocument::compile_locked(&store, &resolution, "natural_equation_oracle")
            .expect("locked package compilation");
    accounting.operation();
    assert!(
        packaged
            .model()
            .structurally_equivalent(&natural_model)
            .expect("package structural comparison")
    );
    accounting.operation();
    assert_eq!(
        packaged
            .model()
            .structural_fingerprint()
            .expect("package structural fingerprint")
            .to_string(),
        explicit_fingerprint
    );

    // Stage 10: native explicit residual in declaration order, structural comparison only.
    let a = DraftField::new(
        "a",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        Some(4.0),
    );
    let b = DraftField::new(
        "b",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        Some(3.0),
    );
    let c = DraftField::new(
        "c",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        Some(2.0),
    );
    let d = DraftField::new(
        "d",
        eqiora_core::ValueType::scalar(
            eqiora_core::ScalarDomain::Real,
            DimExponents::DIMENSIONLESS,
        ),
        Some(1.0),
    );
    let native_roots = vec![
        a.expression() - b.expression(),
        (a.expression() - (b.expression() - c.expression())) - d.expression(),
        (-a.expression()) - (-b.expression()),
    ];
    let relation = DraftRelation::continuous("balance", native_roots);
    let draft = ModelDraft::new(
        "natural_equation_oracle",
        vec![a.into(), b.into(), c.into(), d.into(), relation.into()],
    )
    .expect("native Model draft");
    accounting.operation();
    let native_model = ModelDocument::define(&draft).expect("native Model definition");
    accounting.operation();
    assert!(
        native_model
            .structurally_equivalent(&natural_model)
            .expect("native structural comparison")
    );
    accounting.operation();
    assert_eq!(
        native_model
            .structural_fingerprint()
            .expect("native structural fingerprint")
            .to_string(),
        explicit_fingerprint
    );

    assert!(accounting.public_operations <= 262);
    assert!(accounting.formatter_bytes <= 8192);
    assert_eq!(accounting.projected_diagnostics, 18);
}
