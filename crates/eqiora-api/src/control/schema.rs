/// Committed JSON Schema for the complete compile/check v2 request/response
/// wire.
pub const COMPILE_V2_SCHEMA_JSON: &str =
    include_str!("../../../../schemas/control/compile-v2.schema.json");

/// Return the deterministic committed compile/check v2 JSON Schema.
///
/// The schema is an independently precommitted oracle promoted by the Model
/// epoch reset. Keeping a second hand-built schema representation in product
/// code would weaken that ownership boundary.
///
/// # Errors
/// This current implementation is infallible; the `Result` preserves the
/// conventional schema-generation API shape for build tooling.
pub fn generated_compile_v2_schema_json() -> Result<String, serde_json::Error> {
    Ok(COMPILE_V2_SCHEMA_JSON.to_owned())
}
