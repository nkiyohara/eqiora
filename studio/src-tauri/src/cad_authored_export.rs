//! Studio-to-Python export adapter for the authored-CAD slice.
//!
//! One deterministic renderer projects an accepted authored graph into the
//! closed readable Python program frozen at
//! `bindings/python/tests/fixtures/studio_python_cad_round_trip/`. The export is
//! derived only from the replayed native graph — never from form strings or
//! client state — so equal graphs render byte-identical source. Requests carry
//! neither a filesystem path nor Python source; saving writes only through the
//! exact path chosen by the integrator-owned native dialog, and the
//! path-taking helpers here are never a command boundary themselves.

use std::path::Path;

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;
use eqiora::geometry::CadAuthoredGraph;
use serde::{Deserialize, Serialize};

use crate::cad_authored;

/// Closed wire for the export payload nested in the Studio bridge envelope.
/// Independently versioned; distinct from `eqiora.studio.cad-authored/v1`.
pub(super) const CAD_AUTHORED_EXPORT_PROTOCOL: &str = "eqiora.studio.cad-authored-python-export/v1";
/// The one suggested filename; there is no filename template or selector.
pub(super) const CAD_AUTHORED_EXPORT_FILE_NAME: &str = "eqiora_authored_cad.py";
/// The one native save-dialog filter, `Python (*.py)`.
pub(super) const CAD_AUTHORED_EXPORT_DIALOG_FILTER: (&str, &[&str]) = ("Python", &["py"]);
/// The generated projection for the two admitted histories stays readable
/// and bounded; a longer rendering is a defect, not a bigger file.
const MAX_SOURCE_BYTES: usize = 4_096;

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// The one closed export request: opaque canonical graph bytes plus the exact
/// authored-graph digest they must replay to. A request can supply neither a
/// path nor source, and unknown fields reject before any replay.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct CadAuthoredExportRequestDto {
    pub(super) protocol: String,
    pub(super) canonical_graph_hex: String,
    pub(super) graph_digest: String,
}

/// Bounded source projection of one accepted authored graph.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredExportRenderDto {
    protocol: &'static str,
    graph_digest: String,
    suggested_file_name: &'static str,
    source_utf8: String,
}

/// Outcome of one native save. Cancellation is a normal explicit outcome; a
/// write error is a diagnostic, never a `saved` report.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CadAuthoredExportSaveDto {
    protocol: &'static str,
    graph_digest: String,
    status: CadAuthoredExportSaveStatus,
}

#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum CadAuthoredExportSaveStatus {
    Saved,
    Cancelled,
}

/// Render the closed deterministic Python projection for one exact graph.
pub(super) fn render_export(
    request: &CadAuthoredExportRequestDto,
) -> Result<CadAuthoredExportRenderDto, Diagnostic> {
    let (graph, digest_hex) = replay_bound_graph(request)?;
    Ok(CadAuthoredExportRenderDto {
        protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
        graph_digest: digest_hex.clone(),
        suggested_file_name: CAD_AUTHORED_EXPORT_FILE_NAME,
        source_utf8: render_python_source(&graph, &digest_hex)?,
    })
}

/// Save the same native rendering through the path the native dialog chose.
/// `None` is normal cancellation and writes nothing. The web client never
/// supplies this path; the integrator-owned dialog wrapper does.
pub(super) fn save_export(
    request: &CadAuthoredExportRequestDto,
    dialog_path: Option<&Path>,
) -> Result<CadAuthoredExportSaveDto, Diagnostic> {
    let (graph, digest_hex) = replay_bound_graph(request)?;
    let status = match dialog_path {
        None => CadAuthoredExportSaveStatus::Cancelled,
        Some(path) => {
            write_source_file(path, &render_python_source(&graph, &digest_hex)?)?;
            CadAuthoredExportSaveStatus::Saved
        }
    };
    Ok(CadAuthoredExportSaveDto {
        protocol: CAD_AUTHORED_EXPORT_PROTOCOL,
        graph_digest: digest_hex,
        status,
    })
}

/// Replay the opaque canonical bytes through the owner and bind them to the
/// requested digest. A stale, foreign, mutated, cross-version, or τ-only
/// substituted graph fails here, before any source or dialog is exposed.
fn replay_bound_graph(
    request: &CadAuthoredExportRequestDto,
) -> Result<(CadAuthoredGraph, String), Diagnostic> {
    if request.protocol != CAD_AUTHORED_EXPORT_PROTOCOL {
        return Err(invalid(
            "unsupported Studio authored-CAD Python-export protocol",
        ));
    }
    let (graph, digest_hex) = cad_authored::replay_canonical_graph(&request.canonical_graph_hex)?;
    if request.graph_digest != digest_hex {
        return Err(invalid(
            "authored-CAD Python export references a stale or foreign graph identity",
        ));
    }
    Ok((graph, digest_hex))
}

/// Eqiora's deterministic scalar spelling: Rust-debug shortest round trip for
/// the graph's canonical finite values, with both IEEE-754 zeroes collapsed to
/// one canonical `0.0`. Integer-valued floats keep `.0` and exponents carry no
/// `+` sign or zero padding. Valid Python literals, not Python `repr`.
fn python_scalar(value: f64) -> String {
    if value == 0.0 {
        "0.0".to_owned()
    } else {
        format!("{value:?}")
    }
}

/// The one closed projection for the two admitted histories. This is a fixed
/// template over the owner's replayed scalars, not a code generator: it calls
/// only the public constructor and the one public immutable cut successor,
/// and ends with the digest guard against incompatible reconstruction.
fn render_python_source(graph: &CadAuthoredGraph, digest_hex: &str) -> Result<String, Diagnostic> {
    let sketch = graph.sketch();
    let (x_lower, x_upper) = sketch.x_bounds_m();
    let (y_lower, y_upper) = sketch.y_bounds_m();
    let mut source = format!(
        "# Generated by Eqiora Studio from an accepted authored CAD graph.\n\
         # Length arguments are coherent-SI metres.\n\
         import eqiora\n\
         \n\
         authored_graph = eqiora.geometry.CadAuthoredGraph.rectangle_extrusion(\n\
         \x20   x_bounds=({}, {}),\n\
         \x20   y_bounds=({}, {}),\n\
         \x20   plane_z={},\n\
         \x20   depth={},\n\
         \x20   modeling_tolerance={},\n\
         )\n",
        python_scalar(x_lower),
        python_scalar(x_upper),
        python_scalar(y_lower),
        python_scalar(y_upper),
        python_scalar(sketch.plane_z_m()),
        python_scalar(graph.extrusion_depth_m()),
        python_scalar(graph.requested_modeling_tolerance_m()),
    );
    if let (Some(center_m), Some(radius_m), Some(tolerance_m)) = (
        graph.cut_center_m(),
        graph.cut_radius_m(),
        graph.requested_boolean_tolerance_m(),
    ) {
        source.push_str(&format!(
            "authored_graph = authored_graph.circular_through_cut(\n\
             \x20   center=({}, {}),\n\
             \x20   radius={},\n\
             \x20   boolean_tolerance={},\n\
             )\n",
            python_scalar(center_m[0]),
            python_scalar(center_m[1]),
            python_scalar(radius_m),
            python_scalar(tolerance_m),
        ));
    }
    source.push_str(&format!(
        "\n\
         _expected_graph_digest = (\n\
         \x20   \"{digest_hex}\"\n\
         )\n\
         if authored_graph.graph_digest != _expected_graph_digest:\n\
         \x20   raise RuntimeError(\"Eqiora authored CAD graph digest mismatch after reconstruction\")\n",
    ));
    if source.len() > MAX_SOURCE_BYTES {
        return Err(invalid(
            "authored-CAD Python export exceeds its bounded source size",
        ));
    }
    Ok(source)
}

/// Bounded exact write of the rendered bytes to one caller-owned path. The
/// diagnostic names only the failure kind; it never reports a saved file.
fn write_source_file(path: &Path, source: &str) -> Result<(), Diagnostic> {
    std::fs::write(path, source.as_bytes()).map_err(|error| {
        invalid(format!(
            "failed to write the exported Python file: {}",
            error.kind()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora::geometry::ConstrainedRectangleV1;
    use std::path::PathBuf;

    // The two precommitted oracle programs; consumed byte-for-byte, never
    // edited or re-derived here.
    const V1_FIXTURE: &str = include_str!(
        "../../../bindings/python/tests/fixtures/studio_python_cad_round_trip/rectangle_extrusion.py"
    );
    const V2_FIXTURE: &str = include_str!(
        "../../../bindings/python/tests/fixtures/studio_python_cad_round_trip/circular_through_cut.py"
    );
    const HOSTILE_CORPUS: &str =
        include_str!("../../../verify/interfaces/studio-python-cad-round-trip/models/hostile.json");

    // Frozen witness scalars and digests copied verbatim from the accepted
    // cases `crates/eqiora-geometry/tests/cad_authored_rectangle_extrusion.rs`
    // and `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs`.
    const V1_DIGEST_HEX: &str = "919545f70118840c04da9715829deb2da947460a51311ebabec6a34038c66f36";
    const V2_DIGEST_HEX: &str = "00acb9494fc7dea8f1f2500d1316cb3315130a965a24179b3eb1b10345058b47";

    fn v1_owner() -> CadAuthoredGraph {
        let sketch = ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap();
        CadAuthoredGraph::new(sketch, 4.0, 1.0e-9).unwrap()
    }

    fn v2_owner() -> CadAuthoredGraph {
        let sketch = ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), 0.0).unwrap();
        CadAuthoredGraph::new(sketch, 0.02, 1.0e-10)
            .unwrap()
            .circular_through_cut([0.02, 0.0], 0.008, 1.0e-9)
            .unwrap()
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn request_for(graph: &CadAuthoredGraph, digest_hex: &str) -> CadAuthoredExportRequestDto {
        CadAuthoredExportRequestDto {
            protocol: CAD_AUTHORED_EXPORT_PROTOCOL.to_owned(),
            canonical_graph_hex: hex(graph.canonical_bytes()),
            graph_digest: digest_hex.to_owned(),
        }
    }

    fn v1_request() -> CadAuthoredExportRequestDto {
        request_for(&v1_owner(), V1_DIGEST_HEX)
    }

    fn v2_request() -> CadAuthoredExportRequestDto {
        request_for(&v2_owner(), V2_DIGEST_HEX)
    }

    fn hostile_corpus() -> serde_json::Value {
        serde_json::from_str(HOSTILE_CORPUS).unwrap()
    }

    fn corpus_string<'a>(mutant: &'a serde_json::Value, field: &str) -> &'a str {
        mutant[field]
            .as_str()
            .unwrap_or_else(|| panic!("corpus mutant omitted string field {field}"))
    }

    fn corpus_usize(mutant: &serde_json::Value, field: &str) -> usize {
        usize::try_from(
            mutant[field]
                .as_u64()
                .unwrap_or_else(|| panic!("corpus mutant omitted integer field {field}")),
        )
        .unwrap()
    }

    /// Caller-owned scratch path; exact write tests never open a dialog.
    fn scratch_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "eqiora-studio-python-export-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn v1_render_equals_the_precommitted_fixture_byte_for_byte() {
        let render = render_export(&v1_request()).unwrap();
        assert_eq!(render.protocol, CAD_AUTHORED_EXPORT_PROTOCOL);
        assert_eq!(render.graph_digest, V1_DIGEST_HEX);
        assert_eq!(render.suggested_file_name, CAD_AUTHORED_EXPORT_FILE_NAME);
        assert_eq!(render.source_utf8, V1_FIXTURE);
    }

    #[test]
    fn v2_render_equals_the_precommitted_fixture_byte_for_byte() {
        let render = render_export(&v2_request()).unwrap();
        assert_eq!(render.graph_digest, V2_DIGEST_HEX);
        assert_eq!(render.source_utf8, V2_FIXTURE);
    }

    #[test]
    fn repeated_render_and_save_are_byte_identical() {
        let first = render_export(&v2_request()).unwrap();
        let second = render_export(&v2_request()).unwrap();
        assert_eq!(first.source_utf8, second.source_utf8);

        let path = scratch_path("repeat.py");
        let saved = save_export(&v2_request(), Some(&path)).unwrap();
        assert_eq!(saved.status, CadAuthoredExportSaveStatus::Saved);
        assert_eq!(saved.graph_digest, V2_DIGEST_HEX);
        assert_eq!(std::fs::read(&path).unwrap(), V2_FIXTURE.as_bytes());
        std::fs::remove_file(&path).unwrap();
    }

    #[test]
    fn signed_zero_equivalent_input_replays_the_one_accepted_identity_and_source() {
        // The owner canonicalizes IEEE-754 signed zero at construction, so a
        // `-0.0` spelling reaches this adapter as the one accepted graph.
        let sketch = ConstrainedRectangleV1::new((-0.04, 0.04), (-0.025, 0.025), -0.0).unwrap();
        let graph = CadAuthoredGraph::new(sketch, 0.02, 1.0e-10)
            .unwrap()
            .circular_through_cut([0.02, -0.0], 0.008, 1.0e-9)
            .unwrap();
        let render = render_export(&request_for(&graph, V2_DIGEST_HEX)).unwrap();
        assert_eq!(render.graph_digest, V2_DIGEST_HEX);
        assert_eq!(render.source_utf8, V2_FIXTURE);
    }

    #[test]
    fn generated_source_stays_inside_its_frozen_program_contract() {
        for render in [
            render_export(&v1_request()).unwrap(),
            render_export(&v2_request()).unwrap(),
        ] {
            let source = render.source_utf8;
            assert!(source.len() <= MAX_SOURCE_BYTES);
            assert!(source.ends_with('\n'));
            assert!(!source.contains('\r'));
            assert!(!source.contains('\u{0}'));
            assert!(!source.starts_with('\u{feff}'));
            for line in source.lines() {
                if line.starts_with("import ") || line.starts_with("from ") {
                    assert_eq!(line, "import eqiora");
                }
            }
            assert!(source.contains("authored_graph = "));
            assert!(!source.contains("decode_canonical"));
        }
    }

    /// Wire field names are the corpus's `renderResponseFields` and
    /// `saveResponseFields`, exactly and exhaustively.
    #[test]
    fn render_and_save_wires_carry_exactly_the_frozen_fields() {
        let render = serde_json::to_value(render_export(&v1_request()).unwrap()).unwrap();
        let mut render_keys: Vec<_> = render.as_object().unwrap().keys().cloned().collect();
        render_keys.sort();
        assert_eq!(
            render_keys,
            ["graphDigest", "protocol", "sourceUtf8", "suggestedFileName"]
        );

        let saved = save_export(&v1_request(), None).unwrap();
        let saved = serde_json::to_value(saved).unwrap();
        let mut save_keys: Vec<_> = saved.as_object().unwrap().keys().cloned().collect();
        save_keys.sort();
        assert_eq!(save_keys, ["graphDigest", "protocol", "status"]);
        assert_eq!(saved["status"], "cancelled");

        let path = scratch_path("fields.py");
        let written = save_export(&v1_request(), Some(&path)).unwrap();
        assert_eq!(serde_json::to_value(written).unwrap()["status"], "saved");
        std::fs::remove_file(&path).unwrap();
    }

    /// Hostile corpus `bridgeRequestMutants` with a serde-visible mutation:
    /// `unknown-field`, `client-path`, and `client-source` reject at decode,
    /// before any DTO exists.
    #[test]
    fn request_wire_rejects_unknown_path_and_source_fields() {
        let request = v1_request();
        let base = serde_json::json!({
            "protocol": request.protocol,
            "canonicalGraphHex": request.canonical_graph_hex,
            "graphDigest": request.graph_digest,
        });
        assert!(serde_json::from_value::<CadAuthoredExportRequestDto>(base.clone()).is_ok());

        let corpus = hostile_corpus();
        let mutants = corpus["bridgeRequestMutants"].as_array().unwrap();
        let ids: Vec<_> = mutants
            .iter()
            .map(|mutant| corpus_string(mutant, "id"))
            .collect();
        assert_eq!(
            ids,
            [
                "unknown-protocol",
                "unknown-field",
                "client-path",
                "client-source",
                "odd-graph-hex",
                "uppercase-graph-hex",
                "oversized-graph-hex",
                "unknown-canonical-wire",
                "duplicate-canonical-field",
                "oversized-canonical-wire",
                "stale-digest",
                "cross-version-substitution",
                "tolerance-only-substitution",
            ]
        );
        for mutant in mutants
            .iter()
            .filter(|mutant| mutant["mutation"] == "add-json-field")
        {
            let field = corpus_string(mutant, "field");
            let mut widened = base.clone();
            widened[field] = mutant["value"].clone();
            assert!(
                serde_json::from_value::<CadAuthoredExportRequestDto>(widened).is_err(),
                "field {field} must reject",
            );
        }
    }

    /// The remaining hostile corpus `bridgeRequestMutants`, each rejecting
    /// before source or a save write is exposed.
    #[test]
    fn every_replay_request_mutant_rejects_before_source_or_write() {
        let corpus = hostile_corpus();
        let mutants = corpus["bridgeRequestMutants"].as_array().unwrap();
        for mutant in mutants
            .iter()
            .filter(|mutant| mutant["mutation"] != "add-json-field")
        {
            let id = corpus_string(mutant, "id");
            let mutation = corpus_string(mutant, "mutation");
            let mut request = v1_request();
            match mutation {
                "replace-protocol" => {
                    request.protocol = corpus_string(mutant, "value").to_owned();
                }
                "replace-canonical-graph-hex" => {
                    request.canonical_graph_hex = corpus_string(mutant, "value").to_owned();
                }
                "replace-canonical-graph-hex-repeat" => {
                    request.canonical_graph_hex =
                        corpus_string(mutant, "value").repeat(corpus_usize(mutant, "repeat"));
                }
                "replace-decoded-canonical-wire" => {
                    request.canonical_graph_hex = hex(corpus_string(mutant, "value").as_bytes());
                }
                "replace-decoded-canonical-substring" => {
                    let wire = String::from_utf8(v1_owner().canonical_bytes().to_vec()).unwrap();
                    let changed = wire.replacen(
                        corpus_string(mutant, "needle"),
                        corpus_string(mutant, "value"),
                        1,
                    );
                    assert_ne!(changed, wire, "mutant {id} must alter the wire");
                    request.canonical_graph_hex = hex(changed.as_bytes());
                }
                "replace-decoded-canonical-wire-repeat" => {
                    let wire =
                        corpus_string(mutant, "value").repeat(corpus_usize(mutant, "repeat"));
                    request.canonical_graph_hex = hex(wire.as_bytes());
                }
                "replace-digest-first-nibble" => {
                    request
                        .graph_digest
                        .replace_range(0..1, corpus_string(mutant, "value"));
                }
                "replace-canonical-graph-with-v2-keep-v1-digest" => {
                    request = v2_request();
                    request.graph_digest = V1_DIGEST_HEX.to_owned();
                }
                "construct-v1-with-modeling-tolerance-keep-v1-digest" => {
                    let sketch =
                        ConstrainedRectangleV1::new((-2.0, 3.0), (-1.0, 2.0), 0.5).unwrap();
                    let graph =
                        CadAuthoredGraph::new(sketch, 4.0, mutant["value"].as_f64().unwrap())
                            .unwrap();
                    request = request_for(&graph, V1_DIGEST_HEX);
                }
                _ => panic!("unsupported bridge corpus mutation {mutation}"),
            }
            assert!(render_export(&request).is_err(), "mutant {id} must reject");
            let path = scratch_path(&format!("mutant-{id}.py"));
            assert!(
                save_export(&request, Some(&path)).is_err(),
                "mutant {id} must reject before saving",
            );
            assert!(!path.exists(), "mutant {id} must write nothing");
        }
    }

    /// Hostile corpus `saveMutants`: `dialog-cancelled` writes nothing and is
    /// a normal outcome; `write-error` is a bounded diagnostic, never `saved`.
    #[test]
    fn cancelled_dialog_writes_nothing_and_write_errors_never_report_saved() {
        let corpus = hostile_corpus();
        let save_mutants = corpus["saveMutants"].as_array().unwrap();
        assert_eq!(
            save_mutants
                .iter()
                .map(|mutant| corpus_string(mutant, "id"))
                .collect::<Vec<_>>(),
            ["dialog-cancelled", "write-error"]
        );
        let cancelled = save_export(&v1_request(), None).unwrap();
        assert_eq!(cancelled.status, CadAuthoredExportSaveStatus::Cancelled);
        assert_eq!(cancelled.graph_digest, V1_DIGEST_HEX);

        let missing_dir = scratch_path("absent-directory").join("unwritable.py");
        let error = save_export(&v1_request(), Some(&missing_dir)).unwrap_err();
        let message = format!("{error:?}");
        assert!(
            message.contains("failed to write the exported Python file"),
            "diagnostic must stay bounded: {message}",
        );
        assert!(!message.contains("saved"));
        assert!(!missing_dir.exists());
    }
}
