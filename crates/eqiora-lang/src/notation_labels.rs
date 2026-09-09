//! Checked structural qualification and safe projections of declaration symbols.

use crate::{Notation, NotationAtom, NotationNode};

mod markup;
mod projection;

/// A declaration-label output profile. Accessibility deliberately drops styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotationProfile {
    /// LaTeX symbol algebra, without a math delimiter or executable input.
    Latex,
    /// Presentation MathML elements, without a surrounding math element.
    MathMl,
    /// Unicode atoms with explicit styles and script boundaries.
    Unicode,
    /// ASCII names and explicit script delimiters, without typographic styles.
    Plain,
    /// Spoken atom names and explicit script boundaries, without styles.
    Speech,
}

/// A bounded generated label, separate from source notation admission.
///
/// Qualification merges lower scripts structurally. Its larger generated budget
/// never changes the source parser's byte, depth, or script limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotationLabel {
    root: NotationNode,
}

impl NotationLabel {
    const MAX_NODES: usize = 512;
    const MAX_OUTPUT: usize = 16_384;

    /// Qualify an admitted symbol with admitted instance symbols and source names.
    /// Returns `None` before copying an oversized result; a caller can substitute
    /// its exact identity label without rejecting the physical Model.
    #[must_use]
    pub fn qualified(base: &Notation, qualifiers: &[&Notation], suffixes: &[&str]) -> Option<Self> {
        Self::from_notation(base).qualify(qualifiers, suffixes)
    }

    /// Retain an admitted declaration symbol unchanged.
    #[must_use]
    pub fn from_notation(base: &Notation) -> Self {
        Self {
            root: base.root().clone(),
        }
    }

    /// Append qualifications to an already bounded generated label.
    #[must_use]
    pub fn qualify(&self, qualifiers: &[&Notation], suffixes: &[&str]) -> Option<Self> {
        let mut nodes = count(self.root());
        for qualifier in qualifiers {
            nodes = nodes.checked_add(count(qualifier.root()))?;
            if nodes > Self::MAX_NODES {
                return None;
            }
        }
        for suffix in suffixes {
            // Each source byte becomes at most two hex digits plus an escape atom.
            nodes = nodes.checked_add(suffix.len().checked_mul(3)?)?;
            if nodes > Self::MAX_NODES {
                return None;
            }
        }
        if nodes + 1 > Self::MAX_NODES {
            return None;
        }
        let mut additions = qualifiers
            .iter()
            .map(|value| value.root().clone())
            .collect::<Vec<_>>();
        for suffix in suffixes {
            additions.extend(name_atoms(suffix));
        }
        let mut root = self.root().clone();
        if !additions.is_empty() {
            root = append_lower(root, additions);
        }
        let label = Self { root };
        // Count projected bytes before allocating any output string.
        [
            NotationProfile::Latex,
            NotationProfile::MathMl,
            NotationProfile::Unicode,
            NotationProfile::Plain,
            NotationProfile::Speech,
        ]
        .into_iter()
        .all(|profile| projection::emitted_len(label.root(), profile) <= Self::MAX_OUTPUT)
        .then_some(label)
    }

    /// A generated identifier label, used when no declared symbol is available.
    /// The input is data, not a TeX program, and is never reparsed as source.
    #[must_use]
    pub fn identifier(identifier: &str) -> Option<Self> {
        if identifier.len() > Self::MAX_NODES - 2
            || identifier
                .bytes()
                .map(|byte| if byte.is_ascii_alphanumeric() { 1 } else { 3 })
                .sum::<usize>()
                > Self::MAX_NODES - 2
        {
            return None;
        }
        Some(Self {
            root: NotationNode::Scripted {
                base: Box::new(NotationNode::Atom(NotationAtom::Latin('q'))),
                subscript: name_atoms(identifier),
                superscript: Vec::new(),
            },
        })
    }

    /// Typed symbol tree with generated qualifications already merged.
    #[must_use]
    pub const fn root(&self) -> &NotationNode {
        &self.root
    }

    /// Project this declaration label, never an equation or executable expression.
    #[must_use]
    pub fn render(&self, profile: NotationProfile) -> String {
        projection::render(&self.root, profile)
    }
}

fn append_lower(node: NotationNode, additions: Vec<NotationNode>) -> NotationNode {
    match node {
        // Style is transparent in accessible profiles: extend its symbol rather
        // than creating a second script owner when that style disappears.
        NotationNode::Styled(style, inner) => {
            NotationNode::Styled(style, Box::new(append_lower(*inner, additions)))
        }
        NotationNode::Scripted {
            base,
            mut subscript,
            superscript,
        } => {
            subscript.extend(additions);
            NotationNode::Scripted {
                base,
                subscript,
                superscript,
            }
        }
        base => NotationNode::Scripted {
            base: Box::new(base),
            subscript: additions,
            superscript: Vec::new(),
        },
    }
}

fn count(node: &NotationNode) -> usize {
    match node {
        NotationNode::Atom(_) | NotationNode::Mark(_) => 1,
        NotationNode::Styled(_, inner) | NotationNode::Accented(_, inner) => 1 + count(inner),
        NotationNode::Scripted {
            base,
            subscript,
            superscript,
        } => {
            1 + count(base)
                + subscript
                    .iter()
                    .chain(superscript)
                    .map(count)
                    .sum::<usize>()
        }
    }
}

fn name_atoms(name: &str) -> Vec<NotationNode> {
    let mut atoms = Vec::new();
    for byte in name.bytes() {
        if byte.is_ascii_alphabetic() {
            atoms.push(NotationNode::Atom(NotationAtom::Latin(char::from(byte))));
        } else if byte.is_ascii_digit() {
            atoms.push(NotationNode::Atom(NotationAtom::Digit(byte - b'0')));
        } else {
            atoms.push(NotationNode::Atom(NotationAtom::Latin('x')));
            for digit in [byte >> 4, byte & 15] {
                atoms.push(NotationNode::Atom(if digit < 10 {
                    NotationAtom::Digit(digit)
                } else {
                    NotationAtom::Latin(char::from(b'a' + digit - 10))
                }));
            }
        }
    }
    atoms
}
