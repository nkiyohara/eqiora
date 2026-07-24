//! Deterministic, resource-bounded compiler diagnostic collection.

use std::collections::BTreeMap;

use eqiora_core::diagnostic::{Code, codes};
use eqiora_core::{Diagnostic, GraphPath, Severity, Span};
use eqiora_lang::{ModelDraft, NativeModelAst, TextRange};

/// Construct one source-spanned compiler diagnostic.
///
/// Source coordinates remain diagnostic provenance; lowering and hierarchy
/// phases consume this owner without acquiring a dependency on each other.
pub(crate) fn source_error(
    code: Code,
    file: &str,
    range: TextRange,
    message: impl Into<String>,
) -> Diagnostic {
    Diagnostic::error(code, message).with_span(Span {
        file: file.to_owned(),
        start: range.start(),
        end: range.end(),
    })
}

/// Replace synthetic native-AST coordinates with one stable declaration path.
pub(crate) fn native_diagnostic(
    draft: &ModelDraft,
    native: &NativeModelAst,
    diagnostic: Diagnostic,
) -> Diagnostic {
    let path = diagnostic
        .source_span()
        .and_then(|span| native.graph_path(TextRange::new(span.start, span.end)))
        .cloned()
        .or_else(|| diagnostic.graph_path().cloned())
        .unwrap_or_else(|| GraphPath::new([draft.name().to_owned()]));
    let suggestion = diagnostic.suggestion().cloned();
    let mut native =
        Diagnostic::error(diagnostic.code(), diagnostic.message()).with_graph_path(path);
    if let Some(suggestion) = suggestion {
        native = native.with_suggestion(suggestion);
    }
    native
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct DiagnosticKey {
    source: Option<(String, u32, u32)>,
    severity: u8,
    code: String,
    message: String,
    graph_path: Option<Vec<String>>,
    suggestion: Option<String>,
}

impl DiagnosticKey {
    fn from_diagnostic(diagnostic: &Diagnostic) -> Self {
        Self {
            source: diagnostic
                .source_span()
                .map(|span| (span.file.clone(), span.start, span.end)),
            severity: match diagnostic.severity() {
                Severity::Error => 0,
                Severity::Warning => 1,
                Severity::Note => 2,
            },
            code: diagnostic.code().to_string(),
            message: diagnostic.message().to_owned(),
            graph_path: diagnostic.graph_path().map(|path| path.segments().to_vec()),
            suggestion: diagnostic
                .suggestion()
                .map(|suggestion| suggestion.summary.clone()),
        }
    }
}

/// Sort diagnostics by their complete public representation.
pub(crate) fn stable_sort(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by_cached_key(DiagnosticKey::from_diagnostic);
}

/// Deterministic top-K collector for one compiler validation phase.
///
/// Keeping the lexicographically earliest diagnostics makes the result
/// independent of producer traversal order. A final explicit diagnostic says
/// when the bounded view omitted additional failures.
pub(crate) struct BoundedDiagnostics {
    limit: usize,
    kept: usize,
    omitted: usize,
    entries: BTreeMap<DiagnosticKey, Vec<Diagnostic>>,
}

impl BoundedDiagnostics {
    pub(crate) fn new(limit: usize) -> Self {
        Self {
            limit,
            kept: 0,
            omitted: 0,
            entries: BTreeMap::new(),
        }
    }

    pub(crate) fn push(&mut self, diagnostic: Diagnostic) {
        let key = DiagnosticKey::from_diagnostic(&diagnostic);
        if self.kept < self.limit {
            self.insert(key, diagnostic);
            return;
        }

        let Some(largest) = self.entries.last_key_value().map(|(key, _)| key.clone()) else {
            self.omitted = self.omitted.saturating_add(1);
            return;
        };
        if key <= largest {
            self.remove_one(&largest);
            self.insert(key, diagnostic);
        }
        self.omitted = self.omitted.saturating_add(1);
    }

    pub(crate) fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        for diagnostic in diagnostics {
            self.push(diagnostic);
        }
    }

    pub(crate) const fn is_empty(&self) -> bool {
        self.kept == 0 && self.omitted == 0
    }

    pub(crate) fn finish(self, phase: &str) -> Vec<Diagnostic> {
        let capacity = self.kept.saturating_add(usize::from(self.omitted != 0));
        let mut diagnostics = Vec::with_capacity(capacity);
        for (_, entries) in self.entries {
            diagnostics.extend(entries);
        }
        if self.omitted != 0 {
            diagnostics.push(Diagnostic::error(
                codes::LANGUAGE_LOWERING_ERROR,
                format!(
                    "{phase} retained the first {} diagnostics and omitted {} additional diagnostics at the configured bound",
                    self.limit, self.omitted
                ),
            ));
        }
        diagnostics
    }

    fn insert(&mut self, key: DiagnosticKey, diagnostic: Diagnostic) {
        self.entries.entry(key).or_default().push(diagnostic);
        self.kept += 1;
    }

    fn remove_one(&mut self, key: &DiagnosticKey) {
        let remove_key = {
            let entries = self
                .entries
                .get_mut(key)
                .expect("largest diagnostic key exists");
            entries.pop();
            entries.is_empty()
        };
        if remove_key {
            self.entries.remove(key);
        }
        self.kept -= 1;
    }
}

#[cfg(test)]
mod tests {
    use eqiora_core::Span;

    use super::*;

    fn diagnostic(file: &str, start: u32) -> Diagnostic {
        Diagnostic::error(codes::LANGUAGE_TYPE_ERROR, format!("error at {start}")).with_span(Span {
            file: file.to_owned(),
            start,
            end: start + 1,
        })
    }

    #[test]
    fn bounded_collection_is_order_independent_and_explicitly_truncated() {
        let input = [
            diagnostic("b.eqi", 2),
            diagnostic("a.eqi", 3),
            diagnostic("a.eqi", 1),
        ];
        let collect = |values: Vec<Diagnostic>| {
            let mut diagnostics = BoundedDiagnostics::new(2);
            diagnostics.extend(values);
            diagnostics.finish("definition validation")
        };
        let forward = collect(input.to_vec());
        let reverse = collect(input.into_iter().rev().collect());
        assert_eq!(forward, reverse);
        assert_eq!(forward[0].source_span().expect("span").start, 1);
        assert_eq!(forward[1].source_span().expect("span").start, 3);
        assert!(forward[2].message().contains("omitted 1"));
    }
}
