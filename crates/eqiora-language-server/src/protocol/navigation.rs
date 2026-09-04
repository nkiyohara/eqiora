use eqiora::api::{EditorDefinition, EditorWorkspaceSnapshot};
use lsp_types::{GotoDefinitionParams, GotoDefinitionResponse, Location, ReferenceParams, Uri};

use super::{ServerState, document};
use crate::lsp_projection::{editor_position, source_range};

pub(super) fn definition(
    params: GotoDefinitionParams,
    state: &ServerState,
) -> Result<Option<GotoDefinitionResponse>, String> {
    let uri = &params.text_document_position_params.text_document.uri;
    document(state, uri)?;
    let Some((workspace, file)) = state.resolved(uri) else {
        return Ok(None);
    };
    let position = editor_position(params.text_document_position_params.position);
    let Some(definition) = workspace.definition_for_reference_at_position(file, position) else {
        return Ok(None);
    };
    Ok(Some(GotoDefinitionResponse::Scalar(definition_location(
        state, workspace, uri, definition,
    )?)))
}

pub(super) fn references(
    params: ReferenceParams,
    state: &ServerState,
) -> Result<Vec<Location>, String> {
    let uri = &params.text_document_position.text_document.uri;
    document(state, uri)?;
    let Some((workspace, file)) = state.resolved(uri) else {
        return Ok(Vec::new());
    };
    let position = editor_position(params.text_document_position.position);
    let Some((target, _source)) = workspace.hover_at_position(file, position) else {
        return Ok(Vec::new());
    };

    let mut locations = Vec::new();
    if params.context.include_declaration {
        locations.push(definition_location(state, workspace, uri, target)?);
    }
    for reference in workspace.references() {
        if same_definition(reference.definition(), target) {
            let reference_uri = state
                .uri_for_file(uri, reference.file())
                .ok_or_else(|| "resolved reference URI is unavailable".to_owned())?;
            let snapshot = workspace
                .document(reference.file())
                .ok_or_else(|| "resolved reference document is unavailable".to_owned())?;
            locations.push(Location::new(
                reference_uri,
                source_range(
                    snapshot,
                    usize::try_from(reference.range().start())
                        .map_err(|_| "reference offset exceeds usize")?,
                    usize::try_from(reference.range().end())
                        .map_err(|_| "reference offset exceeds usize")?,
                )?,
            ));
        }
    }
    Ok(locations)
}

fn same_definition(left: &EditorDefinition, right: &EditorDefinition) -> bool {
    left.namespace() == right.namespace()
        && left.path() == right.path()
        && left.kind() == right.kind()
}

fn definition_location(
    state: &ServerState,
    workspace: &EditorWorkspaceSnapshot,
    source_uri: &Uri,
    definition: &EditorDefinition,
) -> Result<Location, String> {
    let uri = state
        .uri_for_file(source_uri, definition.file())
        .ok_or_else(|| "resolved definition URI is unavailable".to_owned())?;
    let snapshot = workspace
        .document(definition.file())
        .ok_or_else(|| "resolved definition document is unavailable".to_owned())?;
    let range = definition.name_range().unwrap_or(definition.range());
    Ok(Location::new(
        uri,
        source_range(
            snapshot,
            usize::try_from(range.start()).map_err(|_| "definition offset exceeds usize")?,
            usize::try_from(range.end()).map_err(|_| "definition offset exceeds usize")?,
        )?,
    ))
}
