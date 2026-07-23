use core::fmt;

use eqiora_core::{Diagnostic, Severity};
use serde::{Deserialize, Serialize};

/// Stable source family for a v1 control-plane diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlDiagnosticSourceV1 {
    /// The request failed the versioned control contract.
    Control,
    /// The ordinary Eqiora compiler, semantic validator, or artifact boundary
    /// rejected an otherwise valid control request.
    Kernel,
}

/// Stable severity spelling carried over the control wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ControlSeverityV1 {
    /// The command did not produce a model.
    Error,
    /// The command produced a result with an attached warning.
    Warning,
    /// Supporting information for another diagnostic.
    Note,
}

/// Source byte range attached to a control-plane diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlSourceSpanV1 {
    file: String,
    start: u32,
    end: u32,
}

impl ControlSourceSpanV1 {
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

/// Bounded machine-applicable suggestion carried by compile/check v1.
///
/// The present kernel exposes a summary-only patch. Later protocol versions
/// may add explicit edits without changing this immutable v1 shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlPatchV1 {
    summary: String,
}

impl ControlPatchV1 {
    /// Human-readable one-line patch summary.
    #[must_use]
    pub fn summary(&self) -> &str {
        &self.summary
    }
}

/// Transport-neutral projection of one structured Eqiora diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ControlDiagnosticV1 {
    source: ControlDiagnosticSourceV1,
    severity: ControlSeverityV1,
    code: String,
    message: String,
    graph_path: Option<Box<[String]>>,
    span: Option<Box<ControlSourceSpanV1>>,
    patch: Option<Box<ControlPatchV1>>,
}

impl ControlDiagnosticV1 {
    /// Subsystem that admitted the diagnostic.
    #[must_use]
    pub const fn source(&self) -> ControlDiagnosticSourceV1 {
        self.source
    }

    /// Stable severity.
    #[must_use]
    pub const fn severity(&self) -> ControlSeverityV1 {
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
    pub fn span(&self) -> Option<&ControlSourceSpanV1> {
        self.span.as_deref()
    }

    /// Machine-applicable suggestion, when one is available.
    #[must_use]
    pub fn patch(&self) -> Option<&ControlPatchV1> {
        self.patch.as_deref()
    }

    pub(crate) fn invalid_request(message: impl Into<String>) -> Self {
        Self::control("EQ0901", message)
    }

    pub(crate) fn unsupported(message: impl Into<String>) -> Self {
        Self::control("EQ0001", message)
    }

    fn control(code: &str, message: impl Into<String>) -> Self {
        Self {
            source: ControlDiagnosticSourceV1::Control,
            severity: ControlSeverityV1::Error,
            code: code.to_owned(),
            message: message.into(),
            graph_path: None,
            span: None,
            patch: None,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), Self> {
        let valid_code = self.code.len() == 6
            && self.code.as_bytes()[..2].iter().all(u8::is_ascii_uppercase)
            && self.code.as_bytes()[2..].iter().all(u8::is_ascii_digit);
        let valid_path = self
            .graph_path
            .as_deref()
            .is_none_or(|path| path.iter().all(|segment| !segment.is_empty()));
        let valid_span = self.span.as_deref().is_none_or(|span| {
            !span.file.is_empty()
                && !span.file.chars().any(char::is_control)
                && span.end >= span.start
        });
        let valid_patch = self
            .patch
            .as_deref()
            .is_none_or(|patch| !patch.summary.is_empty());
        if valid_code && !self.message.is_empty() && valid_path && valid_span && valid_patch {
            Ok(())
        } else {
            Err(Self::invalid_request(
                "compile/check response contains an invalid structured diagnostic",
            ))
        }
    }
}

impl From<Diagnostic> for ControlDiagnosticV1 {
    fn from(diagnostic: Diagnostic) -> Self {
        let severity = match diagnostic.severity() {
            Severity::Error => ControlSeverityV1::Error,
            Severity::Warning => ControlSeverityV1::Warning,
            Severity::Note => ControlSeverityV1::Note,
        };
        Self {
            source: ControlDiagnosticSourceV1::Kernel,
            severity,
            code: diagnostic.code().to_string(),
            message: diagnostic.message().to_owned(),
            graph_path: diagnostic
                .graph_path()
                .map(|path| path.segments().to_vec().into_boxed_slice()),
            span: diagnostic.source_span().map(|span| {
                Box::new(ControlSourceSpanV1 {
                    file: span.file.clone(),
                    start: span.start,
                    end: span.end,
                })
            }),
            patch: diagnostic.suggestion().map(|patch| {
                Box::new(ControlPatchV1 {
                    summary: patch.summary.clone(),
                })
            }),
        }
    }
}

impl fmt::Display for ControlDiagnosticV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}
