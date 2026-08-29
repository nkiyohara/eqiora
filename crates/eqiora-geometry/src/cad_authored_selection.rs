//! Provider-neutral authored-face provenance and graph-bound durable handles.
//!
//! The public handle is deliberately opaque. Closed v1 and v2 wire
//! vocabularies and the exhaustive face key remain private, so later Rust
//! callers do not spread branching over persisted schema variants.

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::{Deserialize, Serialize};

use crate::canonical::CANONICAL_ENCODING;

const HANDLE_SCHEMA_V1: &str = "eqiora.cad-authored-face-handle-envelope/v1";
const HANDLE_SCHEMA_V2: &str = "eqiora.cad-authored-face-handle-envelope/v2";
const MAX_HANDLE_BYTES: usize = 512;
const HEX_DIGITS: [u8; 16] = *b"0123456789abcdef";

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum FaceKey {
    StartCap,
    EndCap,
    ProfileXLower,
    ProfileXUpper,
    ProfileYLower,
    ProfileYUpper,
    CutWall,
}

impl FaceKey {
    pub(crate) const fn start_cap() -> Self {
        Self::StartCap
    }

    /// Cap at the positive-z extrusion end.
    #[must_use]
    pub(crate) const fn end_cap() -> Self {
        Self::EndCap
    }

    /// Lateral face swept from the x-lower rectangle edge.
    #[must_use]
    pub(crate) const fn profile_x_lower() -> Self {
        Self::ProfileXLower
    }

    /// Lateral face swept from the x-upper rectangle edge.
    #[must_use]
    pub(crate) const fn profile_x_upper() -> Self {
        Self::ProfileXUpper
    }

    /// Lateral face swept from the y-lower rectangle edge.
    #[must_use]
    pub(crate) const fn profile_y_lower() -> Self {
        Self::ProfileYLower
    }

    /// Lateral face swept from the y-upper rectangle edge.
    #[must_use]
    pub(crate) const fn profile_y_upper() -> Self {
        Self::ProfileYUpper
    }

    /// Cylindrical wall created by the admitted circular through-cut.
    #[must_use]
    pub(crate) const fn cut_wall() -> Self {
        Self::CutWall
    }

    /// Stable authored-provenance spelling.
    #[must_use]
    pub(crate) const fn provenance_key(self) -> &'static str {
        match self {
            Self::StartCap => "start-cap",
            Self::EndCap => "end-cap",
            Self::ProfileXLower => "profile-x-lower",
            Self::ProfileXUpper => "profile-x-upper",
            Self::ProfileYLower => "profile-y-lower",
            Self::ProfileYUpper => "profile-y-upper",
            Self::CutWall => "cut-wall",
        }
    }

    pub(crate) fn from_provenance_key(value: &str) -> Option<Self> {
        Self::V2_ALL
            .iter()
            .copied()
            .find(|candidate| candidate.provenance_key() == value)
    }

    pub(crate) const V1_ALL: [Self; 6] = [
        Self::start_cap(),
        Self::end_cap(),
        Self::profile_x_lower(),
        Self::profile_x_upper(),
        Self::profile_y_lower(),
        Self::profile_y_upper(),
    ];

    pub(crate) const V2_ALL: [Self; 7] = [
        Self::start_cap(),
        Self::end_cap(),
        Self::profile_x_lower(),
        Self::profile_x_upper(),
        Self::profile_y_lower(),
        Self::profile_y_upper(),
        Self::cut_wall(),
    ];
}

/// Durable face selection bound to one exact authored-graph digest.
///
/// The owner decodes both frozen handle schemas while preserving their exact
/// canonical bytes.  Resolution always checks the graph digest and admitted
/// selection inventory before returning the selection.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct GeometryFaceHandle {
    authoring_owner: u64,
    graph_digest: [u8; 32],
    selection: FaceKey,
    version: HandleVersion,
    bytes: Vec<u8>,
}

impl GeometryFaceHandle {
    /// Decode either closed handle wire through its exact schema vocabulary.
    ///
    /// # Errors
    /// Returns `EQ0901` for excess bytes, malformed or unknown wire data, or a
    /// digest outside 64 lowercase hexadecimal digits.
    pub(crate) fn decode_for_owner(authoring_owner: u64, bytes: &[u8]) -> Result<Self, Diagnostic> {
        if bytes.len() > MAX_HANDLE_BYTES {
            return Err(invalid(format!(
                "CAD face handle has {} bytes, exceeding the {MAX_HANDLE_BYTES} byte decoder limit",
                bytes.len()
            )));
        }
        if let Ok(wire) = serde_json::from_slice::<WireFaceHandleV1>(bytes)
            && wire.schema == HANDLE_SCHEMA_V1
            && wire.encoding == CANONICAL_ENCODING
        {
            return Self::bind_v1(
                authoring_owner,
                decode_digest(&wire.graph_digest_sha256)?,
                wire.selection.into(),
            );
        }
        if let Ok(wire) = serde_json::from_slice::<WireFaceHandleV2>(bytes)
            && wire.schema == HANDLE_SCHEMA_V2
            && wire.encoding == CANONICAL_ENCODING
        {
            return Self::bind_v2(
                authoring_owner,
                decode_digest(&wire.graph_digest_sha256)?,
                wire.selection.into(),
            );
        }
        Err(invalid("unsupported or malformed CAD face handle wire"))
    }

    /// Exact authored-graph digest this handle is bound to.
    #[must_use]
    pub const fn graph_digest_bytes(&self) -> [u8; 32] {
        self.graph_digest
    }

    /// Stable authored provenance selected by this graph-bound handle.
    #[must_use]
    pub const fn provenance_key(&self) -> &'static str {
        self.selection.provenance_key()
    }

    /// Exact compact canonical JSON bytes.
    #[must_use]
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn bind_v1(
        authoring_owner: u64,
        graph_digest: [u8; 32],
        selection: FaceKey,
    ) -> Result<Self, Diagnostic> {
        let selection = WireFaceSelectionV1::try_from(selection)?;
        let wire = WireFaceHandleV1 {
            schema: HANDLE_SCHEMA_V1.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            graph_digest_sha256: encode_hex(graph_digest),
            selection,
        };
        Self::from_wire(
            authoring_owner,
            graph_digest,
            selection.into(),
            HandleVersion::V1,
            &wire,
        )
    }

    pub(crate) fn bind_v2(
        authoring_owner: u64,
        graph_digest: [u8; 32],
        selection: FaceKey,
    ) -> Result<Self, Diagnostic> {
        let selection = WireFaceSelectionV2::from(selection);
        let wire = WireFaceHandleV2 {
            schema: HANDLE_SCHEMA_V2.to_owned(),
            encoding: CANONICAL_ENCODING.to_owned(),
            graph_digest_sha256: encode_hex(graph_digest),
            selection,
        };
        Self::from_wire(
            authoring_owner,
            graph_digest,
            selection.into(),
            HandleVersion::V2,
            &wire,
        )
    }

    fn from_wire<T: Serialize>(
        authoring_owner: u64,
        graph_digest: [u8; 32],
        selection: FaceKey,
        version: HandleVersion,
        wire: &T,
    ) -> Result<Self, Diagnostic> {
        let bytes = serde_json::to_vec(wire)
            .map_err(|error| invalid(format!("cannot serialize CAD face handle: {error}")))?;
        Ok(Self {
            authoring_owner,
            graph_digest,
            selection,
            version,
            bytes,
        })
    }

    pub(crate) const fn is_v1(&self) -> bool {
        matches!(self.version, HandleVersion::V1)
    }

    pub(crate) const fn face_key(&self) -> FaceKey {
        self.selection
    }

    pub(crate) const fn authoring_owner(&self) -> u64 {
        self.authoring_owner
    }
}

fn decode_digest(text: &str) -> Result<[u8; 32], Diagnostic> {
    decode_hex(text)
        .ok_or_else(|| invalid("CAD face handle digest must be 64 lowercase hexadecimal digits"))
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
    for (target, pair) in digest.iter_mut().zip(bytes.as_chunks::<2>().0.iter()) {
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

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
enum HandleVersion {
    V1,
    V2,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFaceHandleV1 {
    schema: String,
    encoding: String,
    graph_digest_sha256: String,
    selection: WireFaceSelectionV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFaceHandleV2 {
    schema: String,
    encoding: String,
    graph_digest_sha256: String,
    selection: WireFaceSelectionV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireFaceSelectionV1 {
    StartCap,
    EndCap,
    ProfileXLower,
    ProfileXUpper,
    ProfileYLower,
    ProfileYUpper,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WireFaceSelectionV2 {
    StartCap,
    EndCap,
    ProfileXLower,
    ProfileXUpper,
    ProfileYLower,
    ProfileYUpper,
    CutWall,
}

impl TryFrom<FaceKey> for WireFaceSelectionV1 {
    type Error = Diagnostic;

    fn try_from(value: FaceKey) -> Result<Self, Self::Error> {
        match value {
            FaceKey::StartCap => Ok(Self::StartCap),
            FaceKey::EndCap => Ok(Self::EndCap),
            FaceKey::ProfileXLower => Ok(Self::ProfileXLower),
            FaceKey::ProfileXUpper => Ok(Self::ProfileXUpper),
            FaceKey::ProfileYLower => Ok(Self::ProfileYLower),
            FaceKey::ProfileYUpper => Ok(Self::ProfileYUpper),
            FaceKey::CutWall => Err(invalid("cut-wall selection is not admitted by graph v1")),
        }
    }
}

impl From<WireFaceSelectionV1> for FaceKey {
    fn from(value: WireFaceSelectionV1) -> Self {
        match value {
            WireFaceSelectionV1::StartCap => Self::start_cap(),
            WireFaceSelectionV1::EndCap => Self::end_cap(),
            WireFaceSelectionV1::ProfileXLower => Self::profile_x_lower(),
            WireFaceSelectionV1::ProfileXUpper => Self::profile_x_upper(),
            WireFaceSelectionV1::ProfileYLower => Self::profile_y_lower(),
            WireFaceSelectionV1::ProfileYUpper => Self::profile_y_upper(),
        }
    }
}

impl From<FaceKey> for WireFaceSelectionV2 {
    fn from(value: FaceKey) -> Self {
        match value {
            FaceKey::StartCap => Self::StartCap,
            FaceKey::EndCap => Self::EndCap,
            FaceKey::ProfileXLower => Self::ProfileXLower,
            FaceKey::ProfileXUpper => Self::ProfileXUpper,
            FaceKey::ProfileYLower => Self::ProfileYLower,
            FaceKey::ProfileYUpper => Self::ProfileYUpper,
            FaceKey::CutWall => Self::CutWall,
        }
    }
}

impl From<WireFaceSelectionV2> for FaceKey {
    fn from(value: WireFaceSelectionV2) -> Self {
        match value {
            WireFaceSelectionV2::StartCap => Self::start_cap(),
            WireFaceSelectionV2::EndCap => Self::end_cap(),
            WireFaceSelectionV2::ProfileXLower => Self::profile_x_lower(),
            WireFaceSelectionV2::ProfileXUpper => Self::profile_x_upper(),
            WireFaceSelectionV2::ProfileYLower => Self::profile_y_lower(),
            WireFaceSelectionV2::ProfileYUpper => Self::profile_y_upper(),
            WireFaceSelectionV2::CutWall => Self::cut_wall(),
        }
    }
}
