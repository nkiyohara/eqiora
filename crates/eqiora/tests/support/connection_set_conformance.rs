use std::collections::BTreeSet;

use eqiora::sem::KernelProgram;
use eqiora::{Code, Diagnostic, RawId};
use eqiora::{graph::EdgeKind, kernel::KernelNode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ConnectionSetObservation {
    pub(crate) connection: RawId,
    pub(crate) members: BTreeSet<RawId>,
}

pub(crate) fn observe_connection_sets(program: &KernelProgram) -> Vec<ConnectionSetObservation> {
    let mut observations = program
        .nodes()
        .filter_map(|node| {
            let KernelNode::Connection(connection) = node else {
                return None;
            };
            let connection = connection.id().erase();
            let members = program
                .edges()
                .iter()
                .filter(|edge| edge.kind() == EdgeKind::Connects && edge.from() == connection)
                .map(|edge| edge.to())
                .collect();
            Some(ConnectionSetObservation {
                connection,
                members,
            })
        })
        .collect::<Vec<_>>();
    observations.sort_by_key(|observation| observation.connection);
    observations
}

pub(crate) fn connection_containing(
    program: &KernelProgram,
    member: RawId,
) -> Result<ConnectionSetObservation, String> {
    let matching = observe_connection_sets(program)
        .into_iter()
        .filter(|observation| observation.members.contains(&member))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [observation] => Ok(observation.clone()),
        [] => Err(format!(
            "physical endpoint {member:?} belongs to no canonical Connection"
        )),
        _ => Err(format!(
            "physical endpoint {member:?} belongs to multiple canonical Connections"
        )),
    }
}

pub(crate) fn require_diagnostic(diagnostics: &[Diagnostic], code: Code, message_fragment: &str) {
    assert!(
        diagnostics.iter().any(|diagnostic| {
            diagnostic.code() == code && diagnostic.message().contains(message_fragment)
        }),
        "expected {code} containing `{message_fragment}`, received {diagnostics:#?}"
    );
}
