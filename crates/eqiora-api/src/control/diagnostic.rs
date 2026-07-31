use core::fmt;

use eqiora_core::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

use super::{
    MAX_CONTROL_DIAGNOSTIC_MESSAGE_BYTES_V2, MAX_CONTROL_GRAPH_PATH_SEGMENTS_V2,
    MAX_CONTROL_TEXT_MEMBER_BYTES_V2,
};

/// Stable source family for a v2 control-plane diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlDiagnosticSourceV2 {
    /// The request failed the control contract.
    Control,
    /// The ordinary compiler, semantic validator, or artifact boundary
    /// rejected an admitted request.
    Kernel,
}

/// Stable severity spelling carried over the v2 control wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlSeverityV2 {
    /// The command did not produce a Model.
    Error,
    /// The command produced a result with an attached warning.
    Warning,
    /// Supporting information for another diagnostic.
    Note,
}

/// Source byte range attached to a v2 control-plane diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlSourceSpanV2 {
    file: String,
    start: u32,
    end: u32,
}

impl ControlSourceSpanV2 {
    /// Workspace-relative source filename.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Inclusive starting byte offset.
    #[must_use]
    pub const fn start(&self) -> u32 {
        self.start
    }

    /// Exclusive ending byte offset.
    #[must_use]
    pub const fn end(&self) -> u32 {
        self.end
    }
}

/// Bounded summary-only suggestion carried by compile/check v2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPatchV2 {
    summary: String,
}

impl ControlPatchV2 {
    /// Human-readable patch summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// Transport-neutral projection of one structured Eqiora diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlDiagnosticV2 {
    source: ControlDiagnosticSourceV2,
    severity: ControlSeverityV2,
    code: String,
    message: String,
    graph_path: Option<Box<[String]>>,
    span: Option<Box<ControlSourceSpanV2>>,
    patch: Option<Box<ControlPatchV2>>,
}

impl ControlDiagnosticV2 {
    /// Subsystem that admitted the diagnostic.
    #[must_use]
    pub const fn source(&self) -> ControlDiagnosticSourceV2 {
        self.source
    }

    /// Stable severity.
    #[must_use]
    pub const fn severity(&self) -> ControlSeverityV2 {
        self.severity
    }

    /// Stable append-only diagnostic identity.
    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    /// Human-readable explanation.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Structured graph path, when one is available.
    #[must_use]
    pub fn graph_path(&self) -> Option<&[String]> {
        self.graph_path.as_deref()
    }

    /// Source byte range, when one is available.
    #[must_use]
    pub fn span(&self) -> Option<&ControlSourceSpanV2> {
        self.span.as_deref()
    }

    /// Machine-applicable suggestion, when one is available.
    #[must_use]
    pub fn patch(&self) -> Option<&ControlPatchV2> {
        self.patch.as_deref()
    }

    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self::control("EQ0901", message)
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::control("EQ0001", message)
    }

    pub(crate) fn diagnostics_overflow() -> Self {
        Self::invalid_request("compile/check diagnostics exceed the control v2 response limits")
    }

    pub(crate) fn from_kernel(diagnostic: Diagnostic) -> Result<Self, ()> {
        let severity = match diagnostic.severity() {
            Severity::Error => ControlSeverityV2::Error,
            Severity::Warning => ControlSeverityV2::Warning,
            Severity::Note => ControlSeverityV2::Note,
        };
        let value = Self {
            source: ControlDiagnosticSourceV2::Kernel,
            severity,
            code: diagnostic.code().to_string(),
            message: diagnostic.message().to_owned(),
            graph_path: diagnostic
                .graph_path()
                .map(|path| path.segments().to_vec().into_boxed_slice()),
            span: diagnostic.source_span().map(|span| {
                Box::new(ControlSourceSpanV2 {
                    file: span.file.clone(),
                    start: span.start,
                    end: span.end,
                })
            }),
            patch: diagnostic.suggestion().map(|patch| {
                Box::new(ControlPatchV2 {
                    summary: patch.summary.clone(),
                })
            }),
        };
        value.validate().map_err(|_| ())?;
        Ok(value)
    }

    pub(crate) fn validate(&self) -> Result<(), Self> {
        let valid_code = self.code.len() == 6
            && self.code.as_bytes()[..2].iter().all(u8::is_ascii_uppercase)
            && self.code.as_bytes()[2..].iter().all(u8::is_ascii_digit);
        let valid_message =
            bounded_nonempty(&self.message, MAX_CONTROL_DIAGNOSTIC_MESSAGE_BYTES_V2);
        let valid_path = self.graph_path.as_deref().is_none_or(|path| {
            path.len() <= MAX_CONTROL_GRAPH_PATH_SEGMENTS_V2
                && path
                    .iter()
                    .all(|segment| bounded_nonempty(segment, MAX_CONTROL_TEXT_MEMBER_BYTES_V2))
        });
        let valid_span = self.span.as_deref().is_none_or(|span| {
            span.file.chars().count() <= MAX_CONTROL_TEXT_MEMBER_BYTES_V2
                && span.file.len() <= MAX_CONTROL_TEXT_MEMBER_BYTES_V2
                && span.end >= span.start
        });
        let valid_patch = self
            .patch
            .as_deref()
            .is_none_or(|patch| bounded_nonempty(&patch.summary, MAX_CONTROL_TEXT_MEMBER_BYTES_V2));
        if valid_code && valid_message && valid_path && valid_span && valid_patch {
            Ok(())
        } else {
            Err(Self::invalid_request(
                "compile/check response contains an invalid structured diagnostic",
            ))
        }
    }

    fn control(code: &str, message: impl Into<String>) -> Self {
        Self {
            source: ControlDiagnosticSourceV2::Control,
            severity: ControlSeverityV2::Error,
            code: code.to_owned(),
            message: message.into(),
            graph_path: None,
            span: None,
            patch: None,
        }
    }
}

impl fmt::Display for ControlDiagnosticV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

fn bounded_nonempty(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.chars().count() <= maximum && value.len() <= maximum
}
