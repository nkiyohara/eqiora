//! Native declaration diagnostic context and lexical validation.

use super::*;

pub(super) fn native_diagnostic(
    model: &str,
    declaration: &str,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_TYPE_ERROR, message)
        .with_graph_path(GraphPath::new([model.to_owned(), declaration.to_owned()]))
}

pub(super) fn connection_path(connection: &DraftConservingConnection) -> String {
    let mut members = connection
        .ports()
        .iter()
        .map(DraftConservingPort::name)
        .collect::<Vec<_>>();
    members.sort_unstable();
    format!("connection[{}]", members.join(","))
}

pub(super) fn is_language_identifier(value: &str) -> bool {
    let mut bytes = value.bytes();
    matches!(bytes.next(), Some(first) if first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
