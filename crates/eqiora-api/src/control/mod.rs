//! Versioned, client-neutral control-plane contracts.
//!
//! This module owns small command and diagnostic values shared by transport
//! adapters. It deliberately excludes scientific arrays, meshes, Fields, and
//! trajectories. Those values belong to the data plane.

mod compile;
mod compile_response;
mod diagnostic;
mod schema;
#[cfg(test)]
mod tests;

pub use crate::ExactModelCodec;
pub use compile::{COMPILE_COMMAND_V1, COMPILE_FEATURE_V1, CompileFeatureV1, CompileRequestV1};
pub use compile_response::{
    CompileControlExecutionV1, CompileModelDescriptorV1, CompileOutcomeV1, CompileResponseV1,
    execute_compile_v1,
};
pub use diagnostic::{
    ControlDiagnosticSourceV1, ControlDiagnosticV1, ControlPatchV1, ControlSeverityV1,
    ControlSourceSpanV1,
};
pub use schema::{COMPILE_V1_SCHEMA_JSON, generated_compile_v1_schema_json};

/// Exact protocol identity for the first bounded control-plane slice.
pub const CONTROL_PROTOCOL_V1: &str = "eqiora.control/v1";

/// Largest admitted encoded compile request.
///
/// This includes the 8 MiB source ceiling plus bounded envelope metadata.
pub const MAX_COMPILE_REQUEST_BYTES_V1: usize = 8 * 1_024 * 1_024 + 16 * 1_024;

/// Largest admitted encoded compile response.
pub const MAX_COMPILE_RESPONSE_BYTES_V1: usize = 8 * 1_024 * 1_024 + 16 * 1_024;

/// Largest admitted Eqiora Language source in one compile request.
pub const MAX_COMPILE_SOURCE_BYTES_V1: usize = 8 * 1_024 * 1_024;

/// Largest admitted UTF-8 filename in one compile request.
pub const MAX_COMPILE_FILENAME_BYTES_V1: usize = 4_096;

/// Largest admitted request identifier.
pub const MAX_CONTROL_REQUEST_ID_BYTES_V1: usize = 128;

/// Largest pre-normalization required-feature list admitted by v1.
pub const MAX_COMPILE_REQUIRED_FEATURES_V1: usize = 16;
