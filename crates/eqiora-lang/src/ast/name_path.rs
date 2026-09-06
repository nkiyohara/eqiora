//! Private construction of structurally segmented source names.

use super::{NamePath, TextRange};

impl NamePath {
    pub(crate) fn from_parsed_segments(
        segments: impl IntoIterator<Item = String>,
        range: TextRange,
    ) -> Self {
        let mut text = String::new();
        let mut ranges = Vec::new();
        for segment in segments {
            if !text.is_empty() {
                text.push('.');
            }
            let start = text.len();
            text.push_str(&segment);
            ranges.push(start..text.len());
        }
        debug_assert!(!ranges.is_empty(), "a NamePath is nonempty");
        Self {
            text,
            segments: ranges,
            range,
        }
    }

    pub(crate) fn single(name: String, range: TextRange) -> Self {
        Self::from_parsed_segments([name], range)
    }

    pub(crate) fn with_range(mut self, range: TextRange) -> Self {
        self.range = range;
        self
    }
}
