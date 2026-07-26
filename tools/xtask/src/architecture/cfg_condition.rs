//! Normalized conditional-compilation conditions.
//!
//! A glob under `#[cfg(feature = "a")]` forwards a different public surface
//! than the same glob under `#[cfg(feature = "b")]`, so the condition has to be
//! part of what is frozen: otherwise flipping the gate repoints the export for
//! every consumer without moving a byte in the ledger.
//!
//! Comparing the attribute's source text would make the frozen value sensitive
//! to formatting and to the order two `any(...)` arms happen to be written in,
//! which turns a reformat into a false failure. So the predicate is normalized
//! instead: nested `all`/`any` of the same kind are flattened, their operands
//! are sorted and deduplicated, and leaf spacing is fixed.
//!
//! `cfg_attr` can also gate an item — `#[cfg_attr(a, cfg(b))]` expands to a
//! `#[cfg]` this module cannot evaluate. Rather than ignore it, its tokens are
//! frozen verbatim, so any edit to such an attribute moves the identity even
//! though its meaning was never interpreted.

use syn::punctuated::Punctuated;
use syn::{Attribute, Expr, Lit, Meta, Token};

/// The conditions one item's attributes impose, normalized, in source order.
///
/// A `Vec` rather than a single string because an item may carry several
/// attributes, and because the caller accumulates the conditions of every
/// enclosing module around the item's own.
pub(super) fn of(attrs: &[Attribute]) -> Vec<String> {
    attrs.iter().filter_map(term).collect()
}

/// Renders an accumulated condition stack as one comparable string.
///
/// Sorted and deduplicated, because `cfg` conjunction is commutative: moving a
/// glob between two modules that impose the same gates is not a change in what
/// it forwards, and should not read as one.
pub(super) fn describe(conditions: &[String]) -> String {
    let mut terms = conditions.to_vec();
    terms.sort();
    terms.dedup();
    match terms.len() {
        0 => "unconditional".to_owned(),
        1 => format!("cfg({})", terms[0]),
        _ => format!("cfg(all({}))", terms.join(", ")),
    }
}

fn term(attr: &Attribute) -> Option<String> {
    let Meta::List(list) = &attr.meta else {
        return None;
    };
    if list.path.is_ident("cfg") {
        return Some(render(&list_predicate(list)));
    }
    // Not interpreted, only frozen: an uninterpreted gate that cannot change
    // silently is a smaller hole than one that is skipped.
    if list.path.is_ident("cfg_attr") {
        return Some(format!("cfg_attr({})", collapse(&list.tokens.to_string())));
    }
    None
}

/// The subset of the `cfg` grammar that has structure worth normalizing.
/// Anything else — including a malformed predicate — survives as a leaf holding
/// its own token text, so it is still frozen.
enum Predicate {
    Leaf(String),
    All(Vec<Predicate>),
    Any(Vec<Predicate>),
    Not(Box<Predicate>),
}

fn parse(meta: &Meta) -> Predicate {
    match meta {
        Meta::Path(path) => Predicate::Leaf(quote_path(path)),
        Meta::NameValue(pair) => Predicate::Leaf(format!(
            "{} = {}",
            quote_path(&pair.path),
            literal(&pair.value)
        )),
        Meta::List(list) => list_predicate(list),
    }
}

fn list_predicate(list: &syn::MetaList) -> Predicate {
    let kind = quote_path(&list.path);
    let Ok(arguments) = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
    else {
        return Predicate::Leaf(format!("{kind}({})", collapse(&list.tokens.to_string())));
    };
    let mut operands: Vec<Predicate> = arguments.iter().map(parse).collect();

    match kind.as_str() {
        // `cfg(x)` and `all(x)` are the same condition as `x`, so neither may
        // add a layer the other would not have.
        "cfg" | "all" if operands.len() == 1 => operands.remove(0),
        "cfg" | "all" => Predicate::All(operands),
        "any" => Predicate::Any(operands),
        "not" if operands.len() == 1 => Predicate::Not(Box::new(operands.remove(0))),
        _ => Predicate::Leaf(format!(
            "{kind}({})",
            operands.iter().map(render).collect::<Vec<_>>().join(", ")
        )),
    }
}

fn render(predicate: &Predicate) -> String {
    match predicate {
        Predicate::Leaf(text) => text.clone(),
        Predicate::All(operands) => format!("all({})", operand_list(operands, is_all)),
        Predicate::Any(operands) => format!("any({})", operand_list(operands, is_any)),
        Predicate::Not(inner) => format!("not({})", render(inner)),
    }
}

/// Flattens operands of the same connective into this one before sorting, so
/// `all(a, all(b, c))` and `all(a, b, c)` — the same condition — freeze alike.
fn operand_list(operands: &[Predicate], same: fn(&Predicate) -> Option<&Vec<Predicate>>) -> String {
    let mut rendered = Vec::new();
    for operand in operands {
        match same(operand) {
            Some(nested) => rendered.extend(nested.iter().map(render)),
            None => rendered.push(render(operand)),
        }
    }
    rendered.sort();
    rendered.dedup();
    rendered.join(", ")
}

fn is_all(predicate: &Predicate) -> Option<&Vec<Predicate>> {
    match predicate {
        Predicate::All(operands) => Some(operands),
        _ => None,
    }
}

fn is_any(predicate: &Predicate) -> Option<&Vec<Predicate>> {
    match predicate {
        Predicate::Any(operands) => Some(operands),
        _ => None,
    }
}

fn quote_path(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// `cfg` admits only string literals on the right of `=`; anything else fails
/// to compile, so the other shapes need no distinguishing form here.
fn literal(expr: &Expr) -> String {
    match expr {
        Expr::Lit(literal) => match &literal.lit {
            Lit::Str(text) => format!("{:?}", text.value()),
            _ => "<non-string literal>".to_owned(),
        },
        _ => "<non-literal>".to_owned(),
    }
}

/// Token streams print with incidental spacing; a single canonical form keeps a
/// reformat from reading as a repoint.
fn collapse(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn condition(source: &str) -> String {
        let parsed: syn::ItemUse = syn::parse_str(source).expect("test source parses");
        describe(&of(&parsed.attrs))
    }

    #[test]
    fn an_ungated_item_is_unconditional() {
        assert_eq!(condition("pub use a::*;"), "unconditional");
    }

    #[test]
    fn a_feature_gate_is_normalized_to_one_spacing() {
        assert_eq!(
            condition("#[cfg(feature=\"vtu\")] pub use a::*;"),
            "cfg(feature = \"vtu\")"
        );
    }

    #[test]
    fn commutative_arms_freeze_to_the_same_value() {
        assert_eq!(
            condition("#[cfg(any(feature = \"b\", feature = \"a\"))] pub use a::*;"),
            condition("#[cfg(any(feature = \"a\", feature = \"b\"))] pub use a::*;")
        );
    }

    #[test]
    fn a_different_arm_is_a_different_condition() {
        assert_ne!(
            condition("#[cfg(any(feature = \"a\", feature = \"b\"))] pub use a::*;"),
            condition("#[cfg(any(feature = \"a\", feature = \"c\"))] pub use a::*;")
        );
    }

    #[test]
    fn nested_connectives_of_one_kind_are_flattened() {
        assert_eq!(
            condition(
                "#[cfg(all(unix, all(feature = \"a\", target_os = \"linux\")))] pub use a::*;"
            ),
            "cfg(all(feature = \"a\", target_os = \"linux\", unix))"
        );
    }

    #[test]
    fn negation_is_kept_distinct_from_its_operand() {
        assert_ne!(
            condition("#[cfg(not(unix))] pub use a::*;"),
            condition("#[cfg(unix)] pub use a::*;")
        );
    }

    #[test]
    fn several_attributes_conjoin() {
        assert_eq!(
            condition("#[cfg(unix)] #[cfg(feature = \"a\")] pub use a::*;"),
            "cfg(all(feature = \"a\", unix))"
        );
    }

    #[test]
    fn an_uninterpreted_cfg_attr_is_still_frozen_verbatim() {
        let frozen = condition("#[cfg_attr(feature = \"a\", cfg(feature = \"b\"))] pub use a::*;");
        assert!(frozen.starts_with("cfg(cfg_attr("), "{frozen}");
        assert_ne!(
            frozen,
            condition("#[cfg_attr(feature = \"a\", cfg(feature = \"c\"))] pub use a::*;")
        );
    }

    #[test]
    fn enclosing_gates_are_deduplicated_not_repeated() {
        assert_eq!(
            describe(&["feature = \"a\"".to_owned(), "feature = \"a\"".to_owned()]),
            "cfg(feature = \"a\")"
        );
    }
}
