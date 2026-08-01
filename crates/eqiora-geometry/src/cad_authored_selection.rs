//! Closed provenance selection over one authored CAD graph, and the durable
//! handle that binds it to exactly one graph identity.
//!
//! A selection names *how the face was authored* — the profile it caps or the
//! authored rectangle edge it was swept from. It is never an output coordinate,
//! a nearest-geometry match, or a provider-local face index, so no selection
//! survives a change of authored meaning by accident.
//!
//! A handle carries the exact authored-graph digest beside that selection. It
//! encodes to bounded canonical JSON so it can cross a process boundary, and
//! every lookup rejects a digest mismatch before it resolves anything. There is
//! deliberately no weaker same-shape or nearest-geometry rebinding.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::CANONICAL_ENCODING;

const FACE_HANDLE_SCHEMA: &str = "eqiora.cad-authored-face-handle-envelope/v1";

/// Bytes accepted by the handle decoder. The canonical form is fixed-shape, so
/// this bounds the parser rather than expressing a policy.
const MAX_HANDLE_BYTES: usize = 512;

const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

/// One of exactly six faces an authored rectangle extrusion can produce.
///
/// Every variant is authored provenance: the cap grown from the authored
/// profile, the cap grown from the translated profile, or the lateral face
/// swept from one authored rectangle edge.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CadAuthoredFaceSelectionV1 {
    /// Cap closing the authored profile at the sketch plane.
    StartCap,
    /// Cap closing the translated profile at the end of the extrusion.
    EndCap,
    /// Lateral face swept from the authored x-lower profile edge.
    ProfileXLower,
    /// Lateral face swept from the authored x-upper profile edge.
    ProfileXUpper,
    /// Lateral face swept from the authored y-lower profile edge.
    ProfileYLower,
    /// Lateral face swept from the authored y-upper profile edge.
    ProfileYUpper,
}

impl CadAuthoredFaceSelectionV1 {
    /// Every admitted selection, in canonical order.
    pub const ALL: [Self; 6] = [
        Self::StartCap,
        Self::EndCap,
        Self::ProfileXLower,
        Self::ProfileXUpper,
        Self::ProfileYLower,
        Self::ProfileYUpper,
    ];
}

/// Durable selection bound to one exact authored-graph identity.
///
/// The digest cannot be chosen beside a graph: the only in-process constructor
/// is [`crate::CadAuthoredGraphV1::face_handle`]. A handle decoded from bytes
/// may name any digest, which is exactly why every lookup compares that digest
/// with the graph before resolving a face.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct CadAuthoredFaceHandleV1 {
    graph_digest: [u8; 32],
    selection: CadAuthoredFaceSelectionV1,
    bytes: Vec<u8>,
}

impl CadAuthoredFaceHandleV1 {
    /// Decode one bounded canonical face handle.
    ///
    /// Object member order is nonsemantic; duplicate and unknown members, an
    /// unsupported schema or encoding, and a digest that is not exactly 64
    /// lowercase hexadecimal digits all reject.
    ///
    /// # Errors
    /// Returns `EQ0901` for excess bytes, malformed or unknown wire data, or a
    /// digest outside the closed hexadecimal vocabulary.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_HANDLE_BYTES {
            return Err(invalid(format!(
                "CAD face handle has {} bytes, exceeding the {MAX_HANDLE_BYTES} byte decoder limit",
                bytes.len(),
            )));
        }
        let wire: WireFaceHandleV1 = serde_json::from_slice(bytes)
            .map_err(|error| invalid(format!("invalid CAD face handle JSON: {error}")))?;
        if wire.schema != FACE_HANDLE_SCHEMA || wire.encoding != CANONICAL_ENCODING {
            return Err(invalid("unsupported CAD face handle schema or encoding"));
        }
        let Some(graph_digest) = decode_hex(&wire.graph_digest_sha256) else {
            return Err(invalid(
                "CAD face handle digest must be 64 lowercase hexadecimal digits",
            ));
        };
        Self::bind(graph_digest, wire.selection)
    }

    /// Exact authored-graph digest this handle is bound to.
    #[must_use]
    pub const fn graph_digest_bytes(&self) -> [u8; 32] {
        self.graph_digest
    }

    /// Closed authored provenance selected by this handle.
    #[must_use]
    pub const fn selection(&self) -> CadAuthoredFaceSelectionV1 {
        self.selection
    }

    /// Exact compact canonical JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn bind(
        graph_digest: [u8; 32],
        selection: CadAuthoredFaceSelectionV1,
    ) -> Result<Self, Diagnostic> {
        let wire = WireFaceHandleV1 {
            schema: FACE_HANDLE_SCHEMA.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            graph_digest_sha256: encode_hex(graph_digest),
            selection,
        };
        let bytes = serde_json::to_vec(&wire)
            .map_err(|error| invalid(format!("cannot serialize CAD face handle: {error}")))?;
        Ok(Self {
            graph_digest,
            selection,
            bytes,
        })
    }
}

fn encode_hex(digest: [u8; 32]) -> String {
    let mut text = String::with_capacity(64);
    for byte in digest {
        text.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        text.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    text
}

fn decode_hex(text: &str) -> Option<[u8; 32]> {
    let bytes = text.as_bytes();
    if bytes.len() != 64 {
        return None;
    }
    let mut digest = [0_u8; 32];
    for (target, pair) in digest.iter_mut().zip(bytes.chunks_exact(2)) {
        *target = (decode_hex_digit(pair[0])? << 4) | decode_hex_digit(pair[1])?;
    }
    Some(digest)
}

const fn decode_hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFaceHandleV1 {
    schema: String,
    encoding: String,
    graph_digest_sha256: String,
    selection: CadAuthoredFaceSelectionV1,
}

#[cfg(test)]
mod tests {
    use super::*;

    const DIGEST: [u8; 32] = [
        0x00, 0x0f, 0x10, 0xff, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b,
        0x0c, 0x0d, 0x0e, 0x1a, 0x2b, 0x3c, 0x4d, 0x5e, 0x6f, 0x70, 0x81, 0x92, 0xa3, 0xb4, 0xc5,
        0xd6, 0xe7,
    ];

    #[test]
    fn hexadecimal_round_trips_and_rejects_other_spellings() {
        let text = encode_hex(DIGEST);
        assert_eq!(text.len(), 64);
        assert_eq!(decode_hex(&text), Some(DIGEST));
        assert_eq!(decode_hex(&text.to_uppercase()), None);
        assert_eq!(decode_hex(&text[..63]), None);
        assert_eq!(decode_hex(&format!("{text}0")), None);
    }

    #[test]
    fn every_selection_has_one_distinct_canonical_spelling() {
        let spellings = CadAuthoredFaceSelectionV1::ALL
            .map(|selection| serde_json::to_string(&selection).expect("closed enum"));
        assert_eq!(
            spellings,
            [
                "\"start-cap\"",
                "\"end-cap\"",
                "\"profile-x-lower\"",
                "\"profile-x-upper\"",
                "\"profile-y-lower\"",
                "\"profile-y-upper\"",
            ]
            .map(str::to_owned)
        );
    }

    #[test]
    fn handle_bytes_replay_across_a_process_boundary() {
        for selection in CadAuthoredFaceSelectionV1::ALL {
            let handle = CadAuthoredFaceHandleV1::bind(DIGEST, selection).expect("bound handle");
            let decoded = CadAuthoredFaceHandleV1::decode_canonical(handle.canonical_bytes())
                .expect("canonical handle bytes replay");
            assert_eq!(decoded, handle);
            assert_eq!(decoded.graph_digest_bytes(), DIGEST);
            assert_eq!(decoded.selection(), selection);
        }
    }

    #[test]
    fn handle_wire_fails_closed() {
        let handle =
            CadAuthoredFaceHandleV1::bind(DIGEST, CadAuthoredFaceSelectionV1::StartCap).unwrap();
        let canonical = String::from_utf8(handle.canonical_bytes().to_vec()).expect("UTF-8");
        for mutant in [
            canonical.replace("\"selection\":", "\"unknown\":0,\"selection\":"),
            canonical.replace(
                "\"selection\":",
                "\"selection\":\"start-cap\",\"selection\":",
            ),
            canonical.replace("\"start-cap\"", "\"top\""),
            canonical.replace(FACE_HANDLE_SCHEMA, "eqiora.cad-authored-face-handle/v2"),
            canonical.replace(CANONICAL_ENCODING, "eqiora.other-json/v1"),
            canonical.replace(&encode_hex(DIGEST), &encode_hex(DIGEST).to_uppercase()),
            canonical.replace("\"graph_digest_sha256\":", "\"graph_digest\":"),
        ] {
            assert!(
                CadAuthoredFaceHandleV1::decode_canonical(mutant.as_bytes()).is_err(),
                "handle mutant must reject: {mutant}"
            );
        }
        assert!(
            CadAuthoredFaceHandleV1::decode_canonical(&vec![b'{'; MAX_HANDLE_BYTES + 1]).is_err()
        );
    }

    #[test]
    fn member_order_is_nonsemantic_for_a_handle() {
        let handle =
            CadAuthoredFaceHandleV1::bind(DIGEST, CadAuthoredFaceSelectionV1::ProfileYUpper)
                .unwrap();
        let permuted = format!(
            "{{\"selection\":\"profile-y-upper\",\"graph_digest_sha256\":\"{}\",\
             \"encoding\":\"{CANONICAL_ENCODING}\",\"schema\":\"{FACE_HANDLE_SCHEMA}\"}}",
            encode_hex(DIGEST)
        );
        let decoded = CadAuthoredFaceHandleV1::decode_canonical(permuted.as_bytes())
            .expect("member order is nonsemantic");
        assert_eq!(decoded, handle);
        assert_eq!(decoded.canonical_bytes(), handle.canonical_bytes());
    }
}
