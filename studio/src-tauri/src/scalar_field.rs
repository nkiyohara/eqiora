//! Bounded session-local transfer of accepted two-dimensional scalar Fields.
//!
//! This adapter owns neither model meaning nor a durable Field artifact. It
//! retains at most two complete, already accepted host Fields and exposes
//! their exact existing Model/run/Realization identity through a small control
//! descriptor. Values cross IPC separately as bounded little-endian chunks.

use std::collections::VecDeque;
use std::mem::size_of;

use eqiora::api::{
    CartesianFieldOrder, MAX_SCALAR_ELLIPTIC_ENTITY_COUNT, ScalarEllipticRunPlan,
    ScalarFieldLocation, ScalarFieldSummary,
};
use eqiora::artifact::ArtifactDigest;
use eqiora::{DimExponents, RawId};
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri::ipc::Response;

use super::{AppState, DiagnosticDto, ProjectionError, studio_error, valid_run_id};

pub(super) const FIELD_VIEW_PROTOCOL: &str = "eqiora.studio.field-view/v1";
const MAX_RETAINED_FIELDS: usize = 2;
const VALUES_PER_CHUNK: usize = 4_096;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct OpenScalarFieldRequest {
    protocol: String,
    model_digest: String,
    run_id: String,
    plan_key: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct ReadScalarFieldChunkRequest {
    protocol: String,
    model_digest: String,
    run_id: String,
    plan_key: String,
    chunk_index: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct FieldViewEnvelope<T> {
    protocol: &'static str,
    result: Option<T>,
    diagnostics: Vec<DiagnosticDto>,
}

impl<T> FieldViewEnvelope<T> {
    fn success(result: T) -> Self {
        Self {
            protocol: FIELD_VIEW_PROTOCOL,
            result: Some(result),
            diagnostics: Vec::new(),
        }
    }

    fn failure(diagnostic: DiagnosticDto) -> Self {
        Self {
            protocol: FIELD_VIEW_PROTOCOL,
            result: None,
            diagnostics: vec![diagnostic],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScalarFieldDescriptor {
    protocol: &'static str,
    model_digest: String,
    run_id: String,
    plan_key: String,
    field: FieldDescriptor,
    domain: DomainDescriptor,
    grid: GridDescriptor,
    transport: TransportDescriptor,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
struct FieldDescriptor {
    id: String,
    name: String,
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
struct GridDescriptor {
    kind: &'static str,
    logical_shape: [usize; 2],
    order: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct TransportDescriptor {
    kind: &'static str,
    encoding: &'static str,
    values_per_chunk: usize,
    chunk_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScalarFieldIdentity {
    model_digest: String,
    run_id: String,
    plan_key: String,
}

impl ScalarFieldIdentity {
    fn from_open(request: &OpenScalarFieldRequest) -> Result<Self, ProjectionError> {
        validate_request_identity(
            &request.protocol,
            &request.model_digest,
            &request.run_id,
            &request.plan_key,
        )?;
        Ok(Self {
            model_digest: request.model_digest.clone(),
            run_id: request.run_id.clone(),
            plan_key: request.plan_key.clone(),
        })
    }

    fn from_chunk(request: &ReadScalarFieldChunkRequest) -> Result<Self, ProjectionError> {
        validate_request_identity(
            &request.protocol,
            &request.model_digest,
            &request.run_id,
            &request.plan_key,
        )?;
        Ok(Self {
            model_digest: request.model_digest.clone(),
            run_id: request.run_id.clone(),
            plan_key: request.plan_key.clone(),
        })
    }
}

#[derive(Debug)]
pub(super) struct PendingScalarField {
    identity: ScalarFieldIdentity,
    field_id: RawId,
    field_name: String,
    field_dimension: DimExponents,
    domain_id: RawId,
    bounds_m: [[f64; 2]; 2],
    location: ScalarFieldLocation,
    logical_shape: [usize; 2],
}

#[derive(Debug)]
pub(super) struct CachedScalarField {
    identity: ScalarFieldIdentity,
    descriptor: ScalarFieldDescriptor,
    values: Box<[f64]>,
}

#[derive(Debug, Default)]
pub(super) struct ScalarFieldCache {
    entries: VecDeque<CachedScalarField>,
}

impl ScalarFieldCache {
    pub(super) fn retains_run_id(&self, run_id: &str) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.identity.run_id == run_id)
    }

    pub(super) fn insert(&mut self, field: CachedScalarField) -> Result<(), ProjectionError> {
        if self.retains_run_id(&field.identity.run_id) {
            return Err(Box::new(studio_error(
                "ST0007",
                "the field-view cache already retains this run ID",
            )));
        }
        self.entries.push_back(field);
        while self.entries.len() > MAX_RETAINED_FIELDS {
            self.entries.pop_front();
        }
        Ok(())
    }

    fn open(
        &self,
        identity: &ScalarFieldIdentity,
    ) -> Result<ScalarFieldDescriptor, ProjectionError> {
        self.entry(identity).map(|entry| entry.descriptor.clone())
    }

    fn chunk(
        &self,
        identity: &ScalarFieldIdentity,
        chunk_index: u32,
    ) -> Result<Vec<u8>, ProjectionError> {
        let entry = self.entry(identity)?;
        encode_chunk(&entry.values, chunk_index)
    }

    fn entry(&self, identity: &ScalarFieldIdentity) -> Result<&CachedScalarField, ProjectionError> {
        self.entries
            .iter()
            .find(|entry| &entry.identity == identity)
            .ok_or_else(|| {
                Box::new(studio_error(
                    "ST0004",
                    "the requested scalar Field is not retained in this Studio session; run the exact Realization again",
                ))
            })
    }
}

/// Project viewer metadata from the accepted application-owned Field layout.
///
/// Non-2D results deliberately return `None`; their scientific run remains
/// valid but this bounded viewer does not retain their bulk values.
pub(super) fn prepare(
    plan: &ScalarEllipticRunPlan,
    model_digest: String,
    run_id: String,
) -> Result<Option<PendingScalarField>, ProjectionError> {
    let projection = plan.field_projection();
    if projection.spatial_dimension() != 2 {
        return Ok(None);
    }
    if model_digest != plan.model_digest() {
        return Err(Box::new(studio_error(
            "ST0008",
            "field-view Model identity differs from the accepted spatial plan",
        )));
    }
    if projection.order() != CartesianFieldOrder::LastAxisFastest {
        return Err(Box::new(studio_error(
            "ST0008",
            "field-view projection has an unsupported canonical value order",
        )));
    }
    let [x_bounds, y_bounds] = projection.bounds() else {
        return Err(Box::new(studio_error(
            "ST0008",
            "field-view projection did not produce exactly two Cartesian bounds",
        )));
    };
    let [width, height] = projection.logical_shape() else {
        return Err(Box::new(studio_error(
            "ST0008",
            "field-view projection did not produce exactly two logical extents",
        )));
    };
    let field_id = projection.field().erase();
    let field_name = projection
        .preferred_alias()
        .filter(|name| !name.is_empty() && name.encode_utf16().count() <= 128)
        .map_or_else(|| field_id.to_string(), ToOwned::to_owned);

    Ok(Some(PendingScalarField {
        identity: ScalarFieldIdentity {
            model_digest,
            run_id,
            plan_key: plan.key().to_owned(),
        },
        field_id,
        field_name,
        field_dimension: projection.value_dimension(),
        domain_id: projection.domain().erase(),
        bounds_m: [*x_bounds, *y_bounds],
        location: projection.location(),
        logical_shape: [*width, *height],
    }))
}

pub(super) fn accept(
    pending: PendingScalarField,
    summary: ScalarFieldSummary,
    values: Vec<f64>,
) -> Result<CachedScalarField, ProjectionError> {
    if summary.spatial_dimension() != 2
        || summary.location() != pending.location
        || summary.value_count() != values.len()
        || values.len() > MAX_SCALAR_ELLIPTIC_ENTITY_COUNT
        || values.iter().any(|value| !value.is_finite())
    {
        return Err(Box::new(studio_error(
            "ST0008",
            "accepted scalar Field values contradict their bounded 2D summary",
        )));
    }
    let logical_shape = summary.logical_shape();
    let [width, height] = logical_shape else {
        return Err(Box::new(studio_error(
            "ST0008",
            "accepted scalar Field does not have exactly two logical extents",
        )));
    };
    let expected_count = (*width).checked_mul(*height).ok_or_else(|| {
        Box::new(studio_error(
            "ST0008",
            "accepted scalar Field logical shape overflows the local platform",
        ))
    })?;
    if expected_count != values.len() {
        return Err(Box::new(studio_error(
            "ST0008",
            "accepted scalar Field logical shape differs from its value count",
        )));
    }
    if [*width, *height] != pending.logical_shape {
        return Err(Box::new(studio_error(
            "ST0008",
            "accepted scalar Field logical shape differs from its previewed projection",
        )));
    }
    let chunk_count = values.len().div_ceil(VALUES_PER_CHUNK);
    let descriptor = ScalarFieldDescriptor {
        protocol: FIELD_VIEW_PROTOCOL,
        model_digest: pending.identity.model_digest.clone(),
        run_id: pending.identity.run_id.clone(),
        plan_key: pending.identity.plan_key.clone(),
        field: FieldDescriptor {
            id: pending.field_id.to_string(),
            name: pending.field_name,
            dimension: pending.field_dimension.to_string(),
            coherent_si_unit: coherent_si_unit(pending.field_dimension),
            scalar_type: "f64",
            location: match pending.location {
                ScalarFieldLocation::Vertex => "vertex",
                ScalarFieldLocation::CellCenter => "cell-center",
            },
            value_count: values.len(),
            minimum: summary.minimum(),
            maximum: summary.maximum(),
        },
        domain: DomainDescriptor {
            id: pending.domain_id.to_string(),
            bounds_m: pending.bounds_m,
        },
        grid: GridDescriptor {
            kind: "uniform-cartesian-2d",
            logical_shape: pending.logical_shape,
            order: "row-major-last-axis-fastest",
        },
        transport: TransportDescriptor {
            kind: "explicit-owned-host-copy",
            encoding: "f64-le",
            values_per_chunk: VALUES_PER_CHUNK,
            chunk_count,
        },
    };
    Ok(CachedScalarField {
        identity: pending.identity,
        descriptor,
        values: values.into_boxed_slice(),
    })
}

#[tauri::command]
pub(super) fn open_scalar_field_view(
    request: OpenScalarFieldRequest,
    state: State<'_, AppState>,
) -> FieldViewEnvelope<ScalarFieldDescriptor> {
    let identity = match ScalarFieldIdentity::from_open(&request) {
        Ok(identity) => identity,
        Err(diagnostic) => return FieldViewEnvelope::failure(*diagnostic),
    };
    match state.scalar_fields.lock() {
        Ok(cache) => match cache.open(&identity) {
            Ok(descriptor) => FieldViewEnvelope::success(descriptor),
            Err(diagnostic) => FieldViewEnvelope::failure(*diagnostic),
        },
        Err(_) => FieldViewEnvelope::failure(studio_error(
            "ST0001",
            "native scalar Field cache is unavailable",
        )),
    }
}

#[tauri::command]
pub(super) fn read_scalar_field_chunk(
    request: ReadScalarFieldChunkRequest,
    state: State<'_, AppState>,
) -> Result<Response, FieldViewEnvelope<()>> {
    let identity = ScalarFieldIdentity::from_chunk(&request)
        .map_err(|diagnostic| FieldViewEnvelope::<()>::failure(*diagnostic))?;
    let bytes = state
        .scalar_fields
        .lock()
        .map_err(|_| {
            FieldViewEnvelope::failure(studio_error(
                "ST0001",
                "native scalar Field cache is unavailable",
            ))
        })?
        .chunk(&identity, request.chunk_index)
        .map_err(|diagnostic| FieldViewEnvelope::<()>::failure(*diagnostic))?;
    Ok(Response::new(bytes))
}

fn validate_request_identity(
    protocol: &str,
    model_digest: &str,
    run_id: &str,
    plan_key: &str,
) -> Result<(), ProjectionError> {
    if protocol != FIELD_VIEW_PROTOCOL
        || !valid_run_id(run_id)
        || ArtifactDigest::from_hex(model_digest.to_owned()).is_err()
        || ArtifactDigest::from_hex(plan_key.to_owned()).is_err()
    {
        return Err(Box::new(studio_error(
            "ST0002",
            "scalar Field request has an invalid protocol or Model/run/Realization identity",
        )));
    }
    Ok(())
}

fn encode_chunk(values: &[f64], chunk_index: u32) -> Result<Vec<u8>, ProjectionError> {
    let chunk_index = usize::try_from(chunk_index).map_err(|_| {
        Box::new(studio_error(
            "ST0002",
            "scalar Field chunk index exceeds the local platform",
        ))
    })?;
    let chunk_count = values.len().div_ceil(VALUES_PER_CHUNK);
    if chunk_index >= chunk_count {
        return Err(Box::new(studio_error(
            "ST0002",
            "scalar Field chunk index is outside the retained value range",
        )));
    }
    let offset = chunk_index.checked_mul(VALUES_PER_CHUNK).ok_or_else(|| {
        Box::new(studio_error(
            "ST0002",
            "scalar Field chunk offset overflowed",
        ))
    })?;
    let end = offset.saturating_add(VALUES_PER_CHUNK).min(values.len());
    let chunk = &values[offset..end];
    let byte_count = chunk.len().checked_mul(size_of::<f64>()).ok_or_else(|| {
        Box::new(studio_error(
            "ST0002",
            "scalar Field chunk byte count overflowed",
        ))
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(byte_count).map_err(|_| {
        Box::new(studio_error(
            "ST0001",
            "scalar Field chunk allocation exceeds available capacity",
        ))
    })?;
    for value in chunk {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    Ok(bytes)
}

fn coherent_si_unit(dimension: DimExponents) -> String {
    let parts = [
        ("kg", dimension.mass),
        ("m", dimension.length),
        ("s", dimension.time),
        ("A", dimension.current),
        ("K", dimension.temperature),
        ("mol", dimension.amount),
        ("cd", dimension.luminous_intensity),
    ];
    let mut unit = String::new();
    for (symbol, exponent) in parts {
        if exponent == 0 {
            continue;
        }
        if !unit.is_empty() {
            unit.push('·');
        }
        unit.push_str(symbol);
        if exponent != 1 {
            unit.push('^');
            unit.push_str(&exponent.to_string());
        }
    }
    if unit.is_empty() {
        unit.push('1');
    }
    unit
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use eqiora::api::{
        ModelDocument, ScalarEllipticExecutionEnvironment, ScalarEllipticIntent,
        ScalarEllipticMethod,
    };
    use eqiora::realization::RealizationRevision;

    use super::*;

    const POISSON_2D: &str =
        include_str!("../../../verify/numerics/cartesian-poisson-fem-fvm/models/poisson.eqi");
    const POISSON_1D: &str =
        include_str!("../../../verify/numerics/poisson-fem-fvm/models/poisson.eqi");
    const POISSON_3D: &str =
        include_str!("../../../verify/numerics/cartesian-poisson-3d-fem-fvm/models/poisson.eqi");

    fn accepted_field(
        method: ScalarEllipticMethod,
        run_id: &str,
        cells: usize,
    ) -> CachedScalarField {
        let document = ModelDocument::compile("poisson.eqi", POISSON_2D).unwrap();
        let digest = document.digest().unwrap();
        let environment = ScalarEllipticExecutionEnvironment::host_serial();
        let plan = document
            .preview_scalar_elliptic_run(
                ScalarEllipticIntent::new(
                    RealizationRevision::new(1),
                    method,
                    NonZeroUsize::new(cells).unwrap(),
                    NonZeroUsize::MIN,
                ),
                environment,
            )
            .unwrap();
        let pending = prepare(&plan, digest, run_id.to_owned()).unwrap().unwrap();
        let result = document
            .run_scalar_elliptic_plan(plan, environment)
            .unwrap();
        accept(pending, result.field(), result.into_field_values()).unwrap()
    }

    fn identity(field: &CachedScalarField) -> ScalarFieldIdentity {
        field.identity.clone()
    }

    #[test]
    fn descriptors_preserve_exact_fem_and_fvm_metadata() {
        let fem = accepted_field(
            ScalarEllipticMethod::FiniteElement,
            "00000000-0000-4000-8000-000000000001",
            2,
        );
        assert_eq!(fem.descriptor.protocol, FIELD_VIEW_PROTOCOL);
        assert_eq!(fem.descriptor.field.name, "potential");
        assert_eq!(fem.descriptor.field.dimension, "1");
        assert_eq!(fem.descriptor.field.coherent_si_unit, "1");
        assert_eq!(fem.descriptor.field.location, "vertex");
        assert_eq!(fem.descriptor.field.value_count, 9);
        assert_eq!(fem.descriptor.domain.bounds_m, [[0.0, 1.0], [0.0, 1.0]]);
        assert_eq!(fem.descriptor.grid.logical_shape, [3, 3]);
        assert_eq!(fem.descriptor.grid.order, "row-major-last-axis-fastest");
        assert_eq!(fem.descriptor.transport.kind, "explicit-owned-host-copy");
        assert_eq!(fem.descriptor.transport.encoding, "f64-le");
        assert_eq!(fem.descriptor.transport.values_per_chunk, VALUES_PER_CHUNK);
        assert_eq!(fem.descriptor.transport.chunk_count, 1);

        let fvm = accepted_field(
            ScalarEllipticMethod::FiniteVolume,
            "00000000-0000-4000-8000-000000000002",
            2,
        );
        assert_eq!(fvm.descriptor.field.location, "cell-center");
        assert_eq!(fvm.descriptor.field.value_count, 4);
        assert_eq!(fvm.descriptor.grid.logical_shape, [2, 2]);
        assert_eq!(fvm.descriptor.domain.bounds_m, [[0.0, 1.0], [0.0, 1.0]]);
    }

    #[test]
    fn one_and_three_dimensional_runs_do_not_publish_viewer_state() {
        for (file, source) in [
            ("poisson-1d.eqi", POISSON_1D),
            ("poisson-3d.eqi", POISSON_3D),
        ] {
            let document = ModelDocument::compile(file, source).unwrap();
            let digest = document.digest().unwrap();
            let environment = ScalarEllipticExecutionEnvironment::host_serial();
            let plan = document
                .preview_scalar_elliptic_run(
                    ScalarEllipticIntent::new(
                        RealizationRevision::new(1),
                        ScalarEllipticMethod::FiniteVolume,
                        NonZeroUsize::new(2).unwrap(),
                        NonZeroUsize::MIN,
                    ),
                    environment,
                )
                .unwrap();
            assert!(
                prepare(
                    &plan,
                    digest,
                    "00000000-0000-4000-8000-000000000009".to_owned(),
                )
                .unwrap()
                .is_none()
            );
        }
    }

    #[test]
    fn cache_rejects_foreign_identity_and_evicts_the_oldest_field() {
        let first = accepted_field(
            ScalarEllipticMethod::FiniteVolume,
            "00000000-0000-4000-8000-000000000001",
            2,
        );
        let first_identity = identity(&first);
        let second = accepted_field(
            ScalarEllipticMethod::FiniteVolume,
            "00000000-0000-4000-8000-000000000002",
            2,
        );
        let second_identity = identity(&second);
        let third = accepted_field(
            ScalarEllipticMethod::FiniteVolume,
            "00000000-0000-4000-8000-000000000003",
            2,
        );
        let third_identity = identity(&third);
        let mut cache = ScalarFieldCache::default();
        cache.insert(first).unwrap();
        cache.insert(second).unwrap();

        let mut foreign = first_identity.clone();
        foreign.plan_key = "f".repeat(64);
        assert!(cache.open(&foreign).is_err());
        assert!(cache.open(&first_identity).is_ok());

        cache.insert(third).unwrap();
        assert!(cache.open(&first_identity).is_err());
        assert!(cache.open(&second_identity).is_ok());
        assert!(cache.open(&third_identity).is_ok());
    }

    #[test]
    fn chunk_bytes_are_exact_little_endian_with_a_bounded_last_chunk() {
        let values = (0..VALUES_PER_CHUNK + 3)
            .map(|index| index as f64 + 0.25)
            .collect::<Vec<_>>();
        let first = encode_chunk(&values, 0).unwrap();
        assert_eq!(first.len(), VALUES_PER_CHUNK * size_of::<f64>());
        assert_eq!(
            f64::from_le_bytes(first[..8].try_into().unwrap()),
            values[0]
        );
        assert_eq!(
            f64::from_le_bytes(first[first.len() - 8..].try_into().unwrap()),
            values[VALUES_PER_CHUNK - 1]
        );

        let last = encode_chunk(&values, 1).unwrap();
        assert_eq!(last.len(), 3 * size_of::<f64>());
        for (encoded, expected) in last.chunks_exact(8).zip(&values[VALUES_PER_CHUNK..]) {
            assert_eq!(f64::from_le_bytes(encoded.try_into().unwrap()), *expected);
        }
        assert!(encode_chunk(&values, 2).is_err());
        assert!(encode_chunk(&values, u32::MAX).is_err());
    }

    #[test]
    fn cache_rejects_duplicate_retained_run_ids() {
        let first = accepted_field(
            ScalarEllipticMethod::FiniteVolume,
            "00000000-0000-4000-8000-000000000001",
            2,
        );
        let duplicate = accepted_field(
            ScalarEllipticMethod::FiniteElement,
            "00000000-0000-4000-8000-000000000001",
            2,
        );
        let mut cache = ScalarFieldCache::default();
        cache.insert(first).unwrap();
        assert!(cache.insert(duplicate).is_err());
    }

    #[test]
    fn coherent_si_unit_uses_base_symbols_and_signed_exponents() {
        assert_eq!(
            coherent_si_unit(DimExponents {
                mass: 1,
                length: 2,
                time: -3,
                current: -1,
                ..DimExponents::DIMENSIONLESS
            }),
            "kg·m^2·s^-3·A^-1"
        );
    }
}
