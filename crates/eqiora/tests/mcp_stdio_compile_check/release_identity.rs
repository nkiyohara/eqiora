use serde_json::{Value, json};

use super::{CONTRACT_SOURCE, TOOL_DEFINITION_SOURCE};

const FROZEN_VERSION: &str = "0.1.0-alpha.3";

pub(super) fn frozen_expected() -> Value {
    serde_json::from_str(CONTRACT_SOURCE).expect("frozen MCP contract JSON")
}

pub(super) fn assert_frozen_release_identity(contract: &Value) {
    assert_eq!(contract["serverInfo"]["version"], FROZEN_VERSION);
    assert_eq!(
        contract["serverDiscover"]["_meta"]["io.modelcontextprotocol/serverInfo"]["version"],
        FROZEN_VERSION
    );
}

pub(super) fn expected() -> Value {
    let mut expected = frozen_expected();
    for path in [
        "/serverInfo/version",
        "/serverDiscover/_meta/io.modelcontextprotocol~1serverInfo/version",
    ] {
        let version = expected
            .pointer_mut(path)
            .unwrap_or_else(|| panic!("missing MCP server version at `{path}`"));
        assert_eq!(version, FROZEN_VERSION);
        *version = json!(env!("CARGO_PKG_VERSION"));
    }
    expected
}

pub(super) fn tool_definition() -> Value {
    serde_json::from_str(TOOL_DEFINITION_SOURCE).expect("frozen tool-definition JSON")
}
