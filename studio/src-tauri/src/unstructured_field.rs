//! Session-local transfer of one accepted unstructured P1 scalar Field.
//!
//! The API layer has already bound semantic meaning to exact fixed-spatial
//! artifacts. This module retains at most two complete projections and moves
//! their three arrays over IPC without creating another Field interpretation.

use std::collections::VecDeque;
use std::mem::size_of;

use eqiora::api::UnstructuredP1ScalarFieldProjection2d;
use eqiora::artifact::ArtifactDigest;
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Response;

use super::scalar_field::coherent_si_unit;
use super::{AppState, DiagnosticDto, ProjectionError, studio_error};

pub(super) const UNSTRUCTURED_FIELD_VIEW_PROTOCOL: &str =
    "eqiora.studio.unstructured-field-view/v1";
const MAX_RETAINED_FIELDS: usize = 2;
const ITEMS_PER_CHUNK: usize = 4_096;
const CHUNK_MAGIC: [u8; 4] = *b"EQP1";
const CHUNK_HEADER_BYTES: usize = 16;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct OpenUnstructuredFieldRequest {
    protocol: String,
    model_digest: String,
    semantic_revision: String,
    realization_digest: String,
    run_digest: String,
    snapshot_digest: String,
    mesh_digest: String,
    field_id: String,
    domain_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum FieldStream {
    Coordinates,
    Triangles,
    Values,
}

impl FieldStream {
    const fn code(self) -> u8 {
        match self {
            Self::Coordinates => 0,
            Self::Triangles => 1,
            Self::Values => 2,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReadUnstructuredFieldChunkRequest {
    protocol: String,
    model_digest: String,
    semantic_revision: String,
    realization_digest: String,
    run_digest: String,
    snapshot_digest: String,
    mesh_digest: String,
    field_id: String,
    domain_id: String,
    stream: FieldStream,
    chunk_index: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UnstructuredFieldEnvelope<T> {
    protocol: &'static str,
    result: Option<T>,
    diagnostics: Vec<DiagnosticDto>,
}

impl<T> UnstructuredFieldEnvelope<T> {
    fn success(result: T) -> Self {
        Self {
            protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
            result: Some(result),
            diagnostics: Vec::new(),
        }
    }

    fn failure(diagnostic: DiagnosticDto) -> Self {
        Self {
            protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
            result: None,
            diagnostics: vec![diagnostic],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct UnstructuredFieldDescriptor {
    protocol: &'static str,
    model_digest: String,
    semantic_revision: String,
    realization_digest: String,
    run_digest: String,
    snapshot_digest: String,
    mesh_digest: String,
    field: FieldDescriptor,
    domain: DomainDescriptor,
    mesh: MeshDescriptor,
    transport: TransportDescriptor,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldDescriptor {
    id: String,
    dimension: String,
    coherent_si_unit: String,
    scalar_type: &'static str,
    location: &'static str,
    value_count: usize,
    minimum: f64,
    maximum: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct DomainDescriptor {
    id: String,
    bounds_m: [[f64; 2]; 2],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct MeshDescriptor {
    kind: &'static str,
    vertex_count: usize,
    triangle_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransportDescriptor {
    kind: &'static str,
    coordinates: StreamDescriptor,
    triangles: StreamDescriptor,
    values: StreamDescriptor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct StreamDescriptor {
    encoding: &'static str,
    components: usize,
    item_count: usize,
    items_per_chunk: usize,
    chunk_count: usize,
}

impl StreamDescriptor {
    fn new(encoding: &'static str, components: usize, item_count: usize) -> Self {
        Self {
            encoding,
            components,
            item_count,
            items_per_chunk: ITEMS_PER_CHUNK,
            chunk_count: item_count.div_ceil(ITEMS_PER_CHUNK),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnstructuredFieldIdentity {
    model_digest: String,
    semantic_revision: String,
    realization_digest: String,
    run_digest: String,
    snapshot_digest: String,
    mesh_digest: String,
    field_id: String,
    domain_id: String,
}

impl UnstructuredFieldIdentity {
    fn from_projection(projection: &UnstructuredP1ScalarFieldProjection2d) -> Self {
        Self {
            model_digest: projection.model_artifact().to_string(),
            semantic_revision: projection.semantic_revision().to_string(),
            realization_digest: projection.realization_artifact().to_string(),
            run_digest: projection.run_artifact().to_string(),
            snapshot_digest: projection.snapshot_artifact().to_string(),
            mesh_digest: projection.mesh_artifact().to_string(),
            field_id: projection.field().erase().to_string(),
            domain_id: projection.support_domain().erase().to_string(),
        }
    }

    fn from_open(request: &OpenUnstructuredFieldRequest) -> Result<Self, ProjectionError> {
        let identity = Self {
            model_digest: request.model_digest.clone(),
            semantic_revision: request.semantic_revision.clone(),
            realization_digest: request.realization_digest.clone(),
            run_digest: request.run_digest.clone(),
            snapshot_digest: request.snapshot_digest.clone(),
            mesh_digest: request.mesh_digest.clone(),
            field_id: request.field_id.clone(),
            domain_id: request.domain_id.clone(),
        };
        identity.validate(&request.protocol)?;
        Ok(identity)
    }

    fn from_chunk(request: &ReadUnstructuredFieldChunkRequest) -> Result<Self, ProjectionError> {
        let identity = Self {
            model_digest: request.model_digest.clone(),
            semantic_revision: request.semantic_revision.clone(),
            realization_digest: request.realization_digest.clone(),
            run_digest: request.run_digest.clone(),
            snapshot_digest: request.snapshot_digest.clone(),
            mesh_digest: request.mesh_digest.clone(),
            field_id: request.field_id.clone(),
            domain_id: request.domain_id.clone(),
        };
        identity.validate(&request.protocol)?;
        Ok(identity)
    }

    fn validate(&self, protocol: &str) -> Result<(), ProjectionError> {
        let canonical_revision = self
            .semantic_revision
            .parse::<u64>()
            .is_ok_and(|revision| revision.to_string() == self.semantic_revision);
        let digests = [
            &self.model_digest,
            &self.realization_digest,
            &self.run_digest,
            &self.snapshot_digest,
            &self.mesh_digest,
        ];
        if protocol != UNSTRUCTURED_FIELD_VIEW_PROTOCOL
            || !canonical_revision
            || digests
                .into_iter()
                .any(|digest| ArtifactDigest::from_hex(digest.clone()).is_err())
            || !self.field_id.starts_with("Field:")
            || !self.domain_id.starts_with("Domain:")
        {
            return Err(Box::new(studio_error(
                "ST0002",
                "unstructured Field request has an invalid protocol or exact artifact identity",
            )));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct CachedUnstructuredField {
    identity: UnstructuredFieldIdentity,
    descriptor: UnstructuredFieldDescriptor,
    projection: UnstructuredP1ScalarFieldProjection2d,
}

impl CachedUnstructuredField {
    fn new(projection: UnstructuredP1ScalarFieldProjection2d) -> Self {
        let identity = UnstructuredFieldIdentity::from_projection(&projection);
        let vertex_count = projection.vertices_m().len();
        let triangle_count = projection.triangles().len();
        let descriptor = UnstructuredFieldDescriptor {
            protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
            model_digest: identity.model_digest.clone(),
            semantic_revision: identity.semantic_revision.clone(),
            realization_digest: identity.realization_digest.clone(),
            run_digest: identity.run_digest.clone(),
            snapshot_digest: identity.snapshot_digest.clone(),
            mesh_digest: identity.mesh_digest.clone(),
            field: FieldDescriptor {
                id: identity.field_id.clone(),
                dimension: projection.value_dimension().to_string(),
                coherent_si_unit: coherent_si_unit(projection.value_dimension()),
                scalar_type: "f64",
                location: "vertex",
                value_count: projection.values().len(),
                minimum: projection.minimum(),
                maximum: projection.maximum(),
            },
            domain: DomainDescriptor {
                id: identity.domain_id.clone(),
                bounds_m: *projection.bounds_m(),
            },
            mesh: MeshDescriptor {
                kind: "affine-triangle-2d",
                vertex_count,
                triangle_count,
            },
            transport: TransportDescriptor {
                kind: "explicit-owned-host-copy",
                coordinates: StreamDescriptor::new("f64-le", 2, vertex_count),
                triangles: StreamDescriptor::new("u32-le", 3, triangle_count),
                values: StreamDescriptor::new("f64-le", 1, projection.values().len()),
            },
        };
        Self {
            identity,
            descriptor,
            projection,
        }
    }

    fn chunk(&self, stream: FieldStream, chunk_index: u32) -> Result<Vec<u8>, ProjectionError> {
        match stream {
            FieldStream::Coordinates => encode_f64_arrays(
                self.projection.vertices_m(),
                FieldStream::Coordinates,
                chunk_index,
            ),
            FieldStream::Triangles => encode_u32_arrays(
                self.projection.triangles(),
                FieldStream::Triangles,
                chunk_index,
            ),
            FieldStream::Values => {
                encode_f64_values(self.projection.values(), FieldStream::Values, chunk_index)
            }
        }
    }
}

#[derive(Debug, Default)]
pub(super) struct UnstructuredFieldCache {
    entries: VecDeque<CachedUnstructuredField>,
}

impl UnstructuredFieldCache {
    #[allow(
        dead_code,
        reason = "the accepted cylinder workflow is the first production publisher"
    )]
    pub(super) fn insert(
        &mut self,
        projection: UnstructuredP1ScalarFieldProjection2d,
    ) -> Result<(), ProjectionError> {
        let field = CachedUnstructuredField::new(projection);
        retain_unique_bounded(
            &mut self.entries,
            field,
            MAX_RETAINED_FIELDS,
            |retained, candidate| retained.identity == candidate.identity,
        )
    }

    fn open(
        &self,
        identity: &UnstructuredFieldIdentity,
    ) -> Result<UnstructuredFieldDescriptor, ProjectionError> {
        Ok(self.entry(identity)?.descriptor.clone())
    }

    fn chunk(
        &self,
        identity: &UnstructuredFieldIdentity,
        stream: FieldStream,
        chunk_index: u32,
    ) -> Result<Vec<u8>, ProjectionError> {
        self.entry(identity)?.chunk(stream, chunk_index)
    }

    fn entry(
        &self,
        identity: &UnstructuredFieldIdentity,
    ) -> Result<&CachedUnstructuredField, ProjectionError> {
        self.entries
            .iter()
            .find(|entry| &entry.identity == identity)
            .ok_or_else(|| {
                Box::new(studio_error(
                    "ST0004",
                    "the requested unstructured Field is not retained in this Studio session",
                ))
            })
    }
}

fn retain_unique_bounded<T>(
    entries: &mut VecDeque<T>,
    candidate: T,
    maximum: usize,
    same_identity: impl Fn(&T, &T) -> bool,
) -> Result<(), ProjectionError> {
    debug_assert!(maximum > 0);
    if entries
        .iter()
        .any(|retained| same_identity(retained, &candidate))
    {
        return Err(Box::new(studio_error(
            "ST0007",
            "the unstructured field-view cache already retains this exact projection",
        )));
    }
    entries.push_back(candidate);
    while entries.len() > maximum {
        entries.pop_front();
    }
    Ok(())
}

#[tauri::command]
pub(super) fn open_unstructured_field_view(
    request: OpenUnstructuredFieldRequest,
    state: State<'_, AppState>,
) -> UnstructuredFieldEnvelope<UnstructuredFieldDescriptor> {
    let identity = match UnstructuredFieldIdentity::from_open(&request) {
        Ok(identity) => identity,
        Err(diagnostic) => return UnstructuredFieldEnvelope::failure(*diagnostic),
    };
    match state.unstructured_fields.lock() {
        Ok(cache) => match cache.open(&identity) {
            Ok(descriptor) => UnstructuredFieldEnvelope::success(descriptor),
            Err(diagnostic) => UnstructuredFieldEnvelope::failure(*diagnostic),
        },
        Err(_) => UnstructuredFieldEnvelope::failure(studio_error(
            "ST0001",
            "native unstructured Field cache is unavailable",
        )),
    }
}

#[tauri::command]
pub(super) fn read_unstructured_field_chunk(
    request: ReadUnstructuredFieldChunkRequest,
    state: State<'_, AppState>,
) -> Result<Response, UnstructuredFieldEnvelope<()>> {
    let identity = UnstructuredFieldIdentity::from_chunk(&request)
        .map_err(|diagnostic| UnstructuredFieldEnvelope::<()>::failure(*diagnostic))?;
    let bytes = state
        .unstructured_fields
        .lock()
        .map_err(|_| {
            UnstructuredFieldEnvelope::failure(studio_error(
                "ST0001",
                "native unstructured Field cache is unavailable",
            ))
        })?
        .chunk(&identity, request.stream, request.chunk_index)
        .map_err(|diagnostic| UnstructuredFieldEnvelope::<()>::failure(*diagnostic))?;
    Ok(Response::new(bytes))
}

fn chunk_range(
    item_count: usize,
    chunk_index: u32,
) -> Result<std::ops::Range<usize>, ProjectionError> {
    let chunk_index = usize::try_from(chunk_index).map_err(|_| {
        Box::new(studio_error(
            "ST0002",
            "unstructured Field chunk index exceeds the local platform",
        ))
    })?;
    let chunk_count = item_count.div_ceil(ITEMS_PER_CHUNK);
    if chunk_index >= chunk_count {
        return Err(Box::new(studio_error(
            "ST0002",
            "unstructured Field chunk index is outside the retained stream",
        )));
    }
    let start = chunk_index.checked_mul(ITEMS_PER_CHUNK).ok_or_else(|| {
        Box::new(studio_error(
            "ST0002",
            "unstructured Field chunk offset overflowed",
        ))
    })?;
    Ok(start..start.saturating_add(ITEMS_PER_CHUNK).min(item_count))
}

fn bytes_with_capacity(
    item_count: usize,
    components: usize,
    scalar_bytes: usize,
) -> Result<Vec<u8>, ProjectionError> {
    let payload_bytes = item_count
        .checked_mul(components)
        .and_then(|count| count.checked_mul(scalar_bytes))
        .ok_or_else(|| {
            Box::new(studio_error(
                "ST0002",
                "unstructured Field chunk byte count overflowed",
            ))
        })?;
    let byte_count = CHUNK_HEADER_BYTES
        .checked_add(payload_bytes)
        .ok_or_else(|| {
            Box::new(studio_error(
                "ST0002",
                "unstructured Field chunk byte count overflowed",
            ))
        })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_count).map_err(|_| {
        Box::new(studio_error(
            "ST0001",
            "unstructured Field chunk allocation exceeds available capacity",
        ))
    })?;
    Ok(bytes)
}

fn write_chunk_header(
    bytes: &mut Vec<u8>,
    stream: FieldStream,
    chunk_index: u32,
    item_count: usize,
) -> Result<(), ProjectionError> {
    let item_count = u32::try_from(item_count).map_err(|_| {
        Box::new(studio_error(
            "ST0002",
            "unstructured Field chunk item count exceeds portable u32",
        ))
    })?;
    bytes.extend_from_slice(&CHUNK_MAGIC);
    bytes.push(stream.code());
    bytes.extend_from_slice(&[0; 3]);
    bytes.extend_from_slice(&chunk_index.to_le_bytes());
    bytes.extend_from_slice(&item_count.to_le_bytes());
    Ok(())
}

fn encode_f64_arrays<const N: usize>(
    values: &[[f64; N]],
    stream: FieldStream,
    chunk_index: u32,
) -> Result<Vec<u8>, ProjectionError> {
    let range = chunk_range(values.len(), chunk_index)?;
    let mut bytes = bytes_with_capacity(range.len(), N, size_of::<f64>())?;
    write_chunk_header(&mut bytes, stream, chunk_index, range.len())?;
    for item in &values[range] {
        for value in item {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn encode_u32_arrays<const N: usize>(
    values: &[[u32; N]],
    stream: FieldStream,
    chunk_index: u32,
) -> Result<Vec<u8>, ProjectionError> {
    let range = chunk_range(values.len(), chunk_index)?;
    let mut bytes = bytes_with_capacity(range.len(), N, size_of::<u32>())?;
    write_chunk_header(&mut bytes, stream, chunk_index, range.len())?;
    for item in &values[range] {
        for value in item {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(bytes)
}

fn encode_f64_values(
    values: &[f64],
    stream: FieldStream,
    chunk_index: u32,
) -> Result<Vec<u8>, ProjectionError> {
    let range = chunk_range(values.len(), chunk_index)?;
    let mut bytes = bytes_with_capacity(range.len(), 1, size_of::<f64>())?;
    write_chunk_header(&mut bytes, stream, chunk_index, range.len())?;
    for value in &values[range] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(byte: char) -> String {
        byte.to_string().repeat(64)
    }

    fn open_request() -> OpenUnstructuredFieldRequest {
        OpenUnstructuredFieldRequest {
            protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL.to_owned(),
            model_digest: digest('0'),
            semantic_revision: "0".to_owned(),
            realization_digest: digest('1'),
            run_digest: digest('2'),
            snapshot_digest: digest('3'),
            mesh_digest: digest('4'),
            field_id: "Field:01HZX3W0A1B2C3D4E5F6G7H8J9".to_owned(),
            domain_id: "Domain:01HZX3W0A1B2C3D4E5F6G7H8JA".to_owned(),
        }
    }

    #[test]
    fn exact_identity_requires_every_artifact_and_semantic_owner() {
        let accepted = open_request();
        assert!(UnstructuredFieldIdentity::from_open(&accepted).is_ok());

        let mut foreign = open_request();
        foreign.snapshot_digest = "not-a-digest".to_owned();
        assert!(UnstructuredFieldIdentity::from_open(&foreign).is_err());

        let mut foreign = open_request();
        foreign.field_id = "Parameter:01HZX3W0A1B2C3D4E5F6G7H8J9".to_owned();
        assert!(UnstructuredFieldIdentity::from_open(&foreign).is_err());

        let mut noncanonical = open_request();
        noncanonical.semantic_revision = "00".to_owned();
        assert!(UnstructuredFieldIdentity::from_open(&noncanonical).is_err());

        let mut maximum = open_request();
        maximum.semantic_revision = u64::MAX.to_string();
        assert!(UnstructuredFieldIdentity::from_open(&maximum).is_ok());
    }

    #[test]
    fn little_endian_stream_chunks_preserve_item_boundaries() {
        let coordinates = [[0.0, -1.0], [2.5, 3.0]];
        let coordinate_bytes =
            encode_f64_arrays(&coordinates, FieldStream::Coordinates, 0).unwrap();
        assert_eq!(
            coordinate_bytes.len(),
            CHUNK_HEADER_BYTES + 4 * size_of::<f64>()
        );
        assert_eq!(&coordinate_bytes[..4], &CHUNK_MAGIC);
        assert_eq!(coordinate_bytes[4], FieldStream::Coordinates.code());
        assert_eq!(
            u32::from_le_bytes(coordinate_bytes[12..16].try_into().unwrap()),
            2
        );
        assert_eq!(
            f64::from_le_bytes(
                coordinate_bytes[CHUNK_HEADER_BYTES + 8..CHUNK_HEADER_BYTES + 16]
                    .try_into()
                    .unwrap()
            ),
            -1.0
        );

        let triangles = [[0, 1, 2], [2, 3, 0]];
        let triangle_bytes = encode_u32_arrays(&triangles, FieldStream::Triangles, 0).unwrap();
        assert_eq!(
            triangle_bytes.len(),
            CHUNK_HEADER_BYTES + 6 * size_of::<u32>()
        );
        assert_eq!(
            u32::from_le_bytes(
                triangle_bytes[CHUNK_HEADER_BYTES + 12..CHUNK_HEADER_BYTES + 16]
                    .try_into()
                    .unwrap()
            ),
            2
        );

        let values = (0..ITEMS_PER_CHUNK + 1)
            .map(|value| value as f64)
            .collect::<Vec<_>>();
        assert_eq!(
            encode_f64_values(&values, FieldStream::Values, 0)
                .unwrap()
                .len(),
            CHUNK_HEADER_BYTES + ITEMS_PER_CHUNK * size_of::<f64>()
        );
        let tail = encode_f64_values(&values, FieldStream::Values, 1).unwrap();
        assert_eq!(
            &tail[CHUNK_HEADER_BYTES..],
            &(ITEMS_PER_CHUNK as f64).to_le_bytes()
        );
        assert!(encode_f64_values(&values, FieldStream::Values, 2).is_err());
    }

    #[test]
    fn bounded_cache_policy_rejects_duplicates_and_evicts_the_oldest_entry() {
        let mut entries = VecDeque::new();
        retain_unique_bounded(&mut entries, 1, 2, PartialEq::eq).unwrap();
        retain_unique_bounded(&mut entries, 2, 2, PartialEq::eq).unwrap();

        let duplicate = retain_unique_bounded(&mut entries, 2, 2, PartialEq::eq).unwrap_err();
        assert_eq!(duplicate.code, "ST0007");
        assert_eq!(entries, VecDeque::from([1, 2]));

        retain_unique_bounded(&mut entries, 3, 2, PartialEq::eq).unwrap();
        assert_eq!(entries, VecDeque::from([2, 3]));
    }
}
