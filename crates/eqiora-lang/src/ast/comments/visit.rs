//! Traverse comment owners in the syntax tree, including signature entries.

use super::SourceComments;
use crate::ast::{ComponentItem, Document, Item, TextRange};

macro_rules! instance {
    ($node:expr, $visit:ident $(, $mutable:tt)?) => {{
        for node in &$($mutable)? $node.bindings {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $node.support_bindings {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $node.boundary_set_bindings {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $node.field_bindings {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $node.property_bindings {
            $visit(node.range, &$($mutable)? node.comments);
        }
    }};
}

macro_rules! owners {
    ($document:expr, $visit:ident $(, $mutable:tt)?) => {{
        for node in &$($mutable)? $document.imports {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $document.dimensions {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $document.property_contracts {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $document.property_releases {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $document.material_compositions {
            $visit(node.range, &$($mutable)? node.comments);
            for property in &$($mutable)? node.properties {
                $visit(property.range, &$($mutable)? property.comments);
            }
        }
        for node in &$($mutable)? $document.connectors {
            $visit(node.range, &$($mutable)? node.comments);
        }
        for node in &$($mutable)? $document.pure_operators {
            $visit(node.range, &$($mutable)? node.comments);
            for formal in &$($mutable)? node.formals {
                $visit(formal.range, &$($mutable)? formal.comments);
            }
        }
        for node in &$($mutable)? $document.components {
            $visit(node.range, &$($mutable)? node.comments);
            for property in &$($mutable)? node.property_requirements {
                $visit(property.range, &$($mutable)? property.comments);
            }
            for form in &$($mutable)? node.formulations {
                $visit(form.range, &$($mutable)? form.comments);
            }
            for item in &$($mutable)? node.items {
                match item {
                    ComponentItem::Parameter(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Port(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::PortFamily(value) => $visit(value.port.range, &$($mutable)? value.port.comments),
                    ComponentItem::Support(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::FieldRequirement(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Representation(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Field(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Initial(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Clock(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::ClockRequirement(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Relation(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::RelationFamily(value) => $visit(value.relation.range, &$($mutable)? value.relation.comments),
                    ComponentItem::Connection(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::BoundaryConnection(value) => $visit(value.range, &$($mutable)? value.comments),
                    ComponentItem::Instance(value) => {
                        $visit(value.range, &$($mutable)? value.comments);
                        instance!(value, $visit $(, $mutable)?);
                    }
                }
            }
        }
        for node in &$($mutable)? $document.models {
            $visit(node.range, &$($mutable)? node.comments);
            for item in &$($mutable)? node.items {
                match item {
                    Item::Domain(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Representation(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Field(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Initial(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Parameter(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Let(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Port(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Clock(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Relation(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Connection(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::BoundaryConnection(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Boundary(value) => $visit(value.range, &$($mutable)? value.comments),
                    Item::Instance(value) => {
                        $visit(value.range, &$($mutable)? value.comments);
                        instance!(value, $visit $(, $mutable)?);
                    }
                }
            }
        }
    }};
}

impl Document {
    /// Attached documentation paired with each declaration's original source range.
    ///
    /// One traversal lets editor and documentation clients index a complete source
    /// without searching the syntax tree separately for every declaration.
    #[must_use]
    pub fn doc_comments(&self) -> impl ExactSizeIterator<Item = (TextRange, &super::DocComment)> {
        let mut comments = Vec::new();
        self.visit_comments(|range, owner| {
            if let Some(doc) = &owner.doc {
                comments.push((range, doc));
            }
        });
        comments.into_iter()
    }

    pub(crate) fn visit_comments<'a>(
        &'a self,
        mut visit: impl FnMut(TextRange, &'a SourceComments),
    ) {
        owners!(self, visit);
    }

    pub(crate) fn visit_comments_mut(
        &mut self,
        mut visit: impl FnMut(TextRange, &mut SourceComments),
    ) {
        owners!(self, visit, mut);
    }

    /// Documentation on the declaration with this exact original source range.
    ///
    /// The lookup is source-local; editor consumers first resolve the declaration
    /// and its source file. Generated source must be parsed for generated ranges.
    #[must_use]
    pub fn doc_comment(&self, declaration: TextRange) -> Option<&super::DocComment> {
        let mut result = None;
        self.visit_comments(|range, comments| {
            if range == declaration {
                result = comments.doc.as_ref();
            }
        });
        result
    }
}

impl Item {
    pub(crate) fn source_comments(&self) -> &SourceComments {
        match self {
            Self::Domain(node) => &node.comments,
            Self::Representation(node) => &node.comments,
            Self::Field(node) => &node.comments,
            Self::Initial(node) => &node.comments,
            Self::Parameter(node) => &node.comments,
            Self::Let(node) => &node.comments,
            Self::Port(node) => &node.comments,
            Self::Clock(node) => &node.comments,
            Self::Relation(node) => &node.comments,
            Self::Connection(node) => &node.comments,
            Self::BoundaryConnection(node) => &node.comments,
            Self::Boundary(node) => &node.comments,
            Self::Instance(node) => &node.comments,
        }
    }
}

impl ComponentItem {
    pub(crate) fn source_comments(&self) -> &SourceComments {
        match self {
            Self::Parameter(node) => &node.comments,
            Self::Port(node) => &node.comments,
            Self::PortFamily(node) => &node.port.comments,
            Self::Support(node) => &node.comments,
            Self::FieldRequirement(node) => &node.comments,
            Self::ClockRequirement(node) => &node.comments,
            Self::Representation(node) => &node.comments,
            Self::Field(node) => &node.comments,
            Self::Initial(node) => &node.comments,
            Self::Clock(node) => &node.comments,
            Self::Relation(node) => &node.comments,
            Self::RelationFamily(node) => &node.relation.comments,
            Self::Connection(node) => &node.comments,
            Self::BoundaryConnection(node) => &node.comments,
            Self::Instance(node) => &node.comments,
        }
    }
}
