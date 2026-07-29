//! Presentation-only command for the verified packaged DC-drive example.

mod core;
mod packages;

use serde::Deserialize;

use self::core::{DcMotorDemoResult, prepare_demo};
use super::{BridgeEnvelope, PROTOCOL, studio_error};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct DcMotorDemoRequest {
    protocol: String,
}

#[tauri::command]
pub(super) async fn run_dc_motor_demo(
    request: DcMotorDemoRequest,
) -> Result<BridgeEnvelope<DcMotorDemoResult>, ()> {
    if request.protocol != PROTOCOL {
        return Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0002",
            "unsupported Studio packaged DC-drive request protocol",
        )]));
    }
    match tauri::async_runtime::spawn_blocking(prepare_demo).await {
        Ok(Ok(result)) => Ok(BridgeEnvelope::success(result)),
        Ok(Err(message)) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0003", message,
        )])),
        Err(error) => Ok(BridgeEnvelope::failure(vec![studio_error(
            "ST0001",
            format!("native packaged DC-drive worker failed: {error}"),
        )])),
    }
}
