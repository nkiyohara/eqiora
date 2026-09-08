use eqiora_lang::parse;

use crate::source_identity::LocalSourceIdentity;

use super::*;

fn validate_source(
    source: &str,
    limits: HierarchyLimits,
) -> Result<CheckedDefinitionGraph, Vec<Diagnostic>> {
    let document = parse("definition-graph.eqi", source)
        .into_compilation_document()
        .expect("definition graph fixture parses");
    let source_identity =
        LocalSourceIdentity::from_document(&document).expect("fixture has canonical identity");
    let elaborator = Elaborator::new(
        "definition-graph.eqi",
        source.len(),
        &document,
        source_identity,
        limits,
    )
    .expect("fixture scopes resolve");
    validate(&elaborator)
}

fn model_summary<'a>(graph: &'a CheckedDefinitionGraph, name: &str) -> &'a DefinitionSummary {
    graph
        .model_summaries
        .iter()
        .find_map(|(key, summary)| (key.display() == name).then_some(summary))
        .expect("Model summary exists")
}

#[test]
fn future_model_depth_boundary_is_exact() {
    let source = "component C2() {} component C1() { instance c2: C2(); } component C0() { instance c1: C1(); } model Main() { instance root: C0(); }";
    let limits = HierarchyLimits {
        max_instance_depth: 4,
        ..HierarchyLimits::default()
    };
    let graph = validate_source(source, limits).expect("Model plus three Components fits");
    assert_eq!(model_summary(&graph, "Main").component_levels(), 4);

    let over = "component C3() {} component C2() { instance c3: C3(); } component C1() { instance c2: C2(); } component C0() { instance c1: C1(); } model Main() { instance root: C0(); }";
    let diagnostics = validate_source(over, limits).expect_err("fifth level fails");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic
            .message()
            .contains("Model-relative instance depth")
            && diagnostic.source_span().is_some()
    }));
}

#[test]
fn cycle_detection_is_independent_of_depth_cutoff() {
    let source = "component C0() { instance c1: C1(); } component C1() { instance c2: C2(); } component C2() { instance c3: C3(); } component C3() { instance c4: C4(); } component C4() { instance c0: C0(); } model Main() {}";
    let limits = HierarchyLimits {
        max_instance_depth: 4,
        ..HierarchyLimits::default()
    };
    let diagnostics = validate_source(source, limits).expect_err("cycle fails");
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.message().contains("recursive component"))
            .count(),
        1
    );
    assert!(diagnostics[0].source_span().is_some());
}

#[test]
fn repeated_definition_edges_retain_occurrence_multiplicity() {
    let source = "component Leaf() {} component Branch() { instance a: Leaf(); instance b: Leaf(); } component Root() { instance x: Branch(); instance y: Branch(); } model Main() { instance root: Root(); }";
    let graph = validate_source(source, HierarchyLimits::default()).expect("DAG is bounded");
    let root = model_summary(&graph, "Main");
    assert_eq!(root.instances(), 7);
    assert_eq!(root.component_levels(), 4);
}

#[test]
fn exponential_occurrence_fails_from_memoized_definition_summary() {
    let source = "component C10() {} component C9() { instance a:C10(); instance b:C10(); } component C8() { instance a:C9(); instance b:C9(); } component C7() { instance a:C8(); instance b:C8(); } component C6() { instance a:C7(); instance b:C7(); } component C5() { instance a:C6(); instance b:C6(); } component C4() { instance a:C5(); instance b:C5(); } component C3() { instance a:C4(); instance b:C4(); } component C2() { instance a:C3(); instance b:C3(); } component C1() { instance a:C2(); instance b:C2(); } component C0() { instance a:C1(); instance b:C1(); } model Main() {}";
    let limits = HierarchyLimits {
        max_instances: 1_000,
        ..HierarchyLimits::default()
    };
    let diagnostics = validate_source(source, limits).expect_err("2^11-1 exceeds bound");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message().contains("Component instances") && diagnostic.source_span().is_some()
    }));
}

#[test]
fn model_edges_share_the_definition_edge_budget() {
    let source = "component Leaf() {} model Main() { instance a:Leaf(); instance b:Leaf(); }";
    let limits = HierarchyLimits {
        max_definition_edges: 1,
        ..HierarchyLimits::default()
    };
    let diagnostics = validate_source(source, limits).expect_err("second Model edge exceeds");
    assert!(diagnostics.iter().any(|diagnostic| {
        diagnostic.message().contains("definition-edge limit") && diagnostic.source_span().is_some()
    }));
}

#[test]
fn indexed_relations_count_default_extent_and_neighbor_declarations() {
    let source = "model Main(parameter n:integer=3) { indexset S=range(n); variable x:1; relation r[i in S] { x=0; } relation neighbor { x=0; } }";
    let graph = validate_source(source, HierarchyLimits::default()).unwrap();
    // Parameter + IndexSet + Field + three (Relation, Activation) pairs + one neighboring pair.
    assert_eq!(model_summary(&graph, "Main").declarations(), 11);
    let errors = validate_source(
        source,
        HierarchyLimits {
            max_declarations: 10,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("11 declarations"))
    );
}

#[test]
fn child_occurrences_multiply_closed_relation_family_work() {
    let source = "component Cell() { indexset S=range(3); variable x:1; relation r[i in S] { x=0; } } model Main() { instance a:Cell(); instance b:Cell(); }";
    let graph = validate_source(source, HierarchyLimits::default()).unwrap();
    assert_eq!(model_summary(&graph, "Main").declarations(), 16);
    assert_eq!(
        model_summary(&graph, "Main").expression_nodes.observed(),
        12
    );
    assert!(
        validate_source(
            source,
            HierarchyLimits {
                max_declarations: 15,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn indexed_connection_families_share_the_neighbor_connection_budget() {
    let source = "component Sink(input u:1) {} model Main(input source:1, output observed:1) { indexset S=range(3); instance sink[i in S]:Sink(); connect [j in S] source -> sink[index(S,ordinal(j))].u; connect source -> observed; }";
    let graph = validate_source(source, HierarchyLimits::default()).unwrap();
    assert_eq!(model_summary(&graph, "Main").connections(), 4);
    let errors = validate_source(
        source,
        HierarchyLimits {
            max_connections: 3,
            ..Default::default()
        },
    )
    .unwrap_err();
    assert!(
        errors
            .iter()
            .any(|error| error.message().contains("4 Connections"))
    );
}
