//! Bounded declaration notation. These nodes are symbols, never expression operators.

mod parse;
mod table;
pub use table::{NotationAccent, NotationAtom, NotationMark, NotationStyle};

use crate::TextRange;

/// One admitted symbol decoration, independent of any rendering backend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotationNode {
    /// A Latin/Greek letter or a named mathematical symbol.
    Atom(NotationAtom),
    /// Explicit presentation style; overrides a future type-derived default.
    Styled(NotationStyle, Box<NotationNode>),
    /// An explicit accent on a symbol.
    Accented(NotationAccent, Box<NotationNode>),
    /// Intrinsic symbol scripts, not indexing or exponentiation of an expression.
    Scripted {
        /// The decorated symbol.
        base: Box<NotationNode>,
        /// Intrinsic lower script, in written order.
        subscript: Vec<NotationNode>,
        /// Intrinsic upper script, in written order.
        superscript: Vec<NotationNode>,
    },
    /// An intrinsic script mark, never an executable operation.
    Mark(NotationMark),
}

/// Validated declaration notation with original source provenance.
///
/// Construction passes through the same bounded parser as source `@{...}` islands.
/// The root is read-only so clients cannot bypass admission or allocation bounds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notation {
    root: NotationNode,
    range: TextRange,
    emitted_bytes: usize,
}

/// A notation error at a UTF-8 byte range relative to the supplied island.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotationError {
    range: TextRange,
    message: &'static str,
}

impl NotationError {
    /// Offending byte range in the supplied `@{...}` string.
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }
    /// Stable explanation of the rejected notation.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for NotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}
impl std::error::Error for NotationError {}

impl Notation {
    pub(crate) const MAX_BYTES: usize = 1024;
    pub(crate) const MAX_TOKENS: usize = 256;
    pub(crate) const MAX_DEPTH: usize = 8;
    pub(crate) const MAX_SCRIPT_ATOMS: usize = 32;
    pub(crate) const MAX_EMITTED_BYTES: usize = 4096;

    /// Parse one complete `@{...}` island, rejecting executable or unbounded TeX.
    pub fn parse(island: &str) -> Result<Self, NotationError> {
        parse::parse(island)
    }

    /// The backend-neutral, admitted symbol algebra.
    #[must_use]
    pub const fn root(&self) -> &NotationNode {
        &self.root
    }

    /// Original island range, relative to its source file (native input starts at zero).
    #[must_use]
    pub const fn range(&self) -> TextRange {
        self.range
    }

    /// One canonical TeX-shaped spelling, including `@{` and `}`.
    #[must_use]
    pub fn canonical(&self) -> String {
        let mut output = String::with_capacity(self.emitted_bytes);
        output.push_str("@{");
        self.root.write(&mut output);
        output.push('}');
        debug_assert_eq!(output.len(), self.emitted_bytes);
        output
    }

    pub(crate) fn at(mut self, range: TextRange) -> Self {
        self.range = range;
        self
    }
}

impl NotationNode {
    fn emitted_bytes(&self) -> usize {
        match self {
            Self::Atom(atom) => atom.spelling().len(),
            Self::Mark(mark) => mark.spelling().len(),
            Self::Styled(style, inner) => style.spelling().len() + 2 + inner.emitted_bytes(),
            Self::Accented(accent, inner) => accent.spelling().len() + 2 + inner.emitted_bytes(),
            Self::Scripted {
                base,
                subscript,
                superscript,
            } => {
                base.emitted_bytes()
                    + [subscript, superscript]
                        .into_iter()
                        .map(|script| {
                            if script.is_empty() {
                                0
                            } else {
                                3 + script.iter().map(Self::emitted_bytes).sum::<usize>()
                                    + script.len()
                                    - 1
                            }
                        })
                        .sum::<usize>()
            }
        }
    }

    fn write(&self, output: &mut String) {
        match self {
            Self::Atom(atom) => output.push_str(&atom.spelling()),
            Self::Mark(mark) => output.push_str(mark.spelling()),
            Self::Styled(style, inner) => {
                output.push_str(style.spelling());
                output.push('{');
                inner.write(output);
                output.push('}');
            }
            Self::Accented(accent, inner) => {
                output.push_str(accent.spelling());
                output.push('{');
                inner.write(output);
                output.push('}');
            }
            Self::Scripted {
                base,
                subscript,
                superscript,
            } => {
                base.write(output);
                for (marker, script) in [('_', subscript), ('^', superscript)] {
                    if !script.is_empty() {
                        output.push(marker);
                        output.push('{');
                        for (index, node) in script.iter().enumerate() {
                            if index != 0 {
                                output.push(' ');
                            }
                            node.write(output);
                        }
                        output.push('}');
                    }
                }
            }
        }
    }
}
