use eqiora_compiler::compile;
use eqiora_graph::Op;
use eqiora_schema::kernel::KernelNode;

#[test]
fn indexed_relations_expand_in_models_and_components() {
    for source in [
        "model M(){indexset I=range(3);relation r[i in I]{to_real(ordinal(i))=to_real(ordinal(i));}}",
        "component C(){indexset I=range(3);relation r[i in I]{to_real(ordinal(i))=to_real(ordinal(i));}} model M(){instance c:C();}",
    ] {
        let models =
            compile("indexed-equations.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
        assert_eq!(
            models[0]
                .transaction()
                .ops()
                .iter()
                .filter(|op| matches!(
                    op,
                    Op::DefineKernelNode {
                        node: KernelNode::Relation(_)
                    }
                ))
                .count(),
            3
        );
    }
}

#[test]
fn indexed_signal_connections_keep_exact_members() {
    let source = "component Drive(parameter value:integer,output y:integer){relation r{y=value;}} component Sink(input u:integer,output y:integer){relation r{y=u;}} model M(){indexset I=range(3);instance drive[i in I]:Drive(value=ordinal(i));instance sink[i in I]:Sink();connect [j in I] drive[index(I,ordinal(j))].y -> sink[index(I,ordinal(j))].u;}";
    let models =
        compile("indexed-connections.eqi", source).unwrap_or_else(|errors| panic!("{errors:?}"));
    assert_eq!(
        models[0]
            .transaction()
            .ops()
            .iter()
            .filter(|op| matches!(
                op,
                Op::DefineKernelNode {
                    node: KernelNode::Connection(_)
                }
            ))
            .count(),
        3
    );
}
