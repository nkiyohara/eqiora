use eqiora_core::Diagnostic;
use eqiora_lang::NamePath;

use super::{Budget, Encoder, as_u32, encode_name, encode_path, source_identity_error};

// Authored paths contain at most 256 segments, so the high bit is an
// unambiguous structural discriminant rather than a name-like sentinel.
const RESOLVED_TYPE_PATH_DISCRIMINANT: u32 = 1 << 31;

/// One already-resolved semantic target substituted for an authored alias.
///
/// Package ownership and logical-module identity remain distinct fields so
/// their canonical encoding cannot depend on separator spellings.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ResolvedAliasTarget {
    LocalModule {
        module: Box<[String]>,
    },
    ExternalModule {
        owner: Box<[String]>,
        module: Box<[String]>,
    },
}

impl ResolvedAliasTarget {
    pub(crate) fn local_module(module: &[String]) -> Self {
        Self::LocalModule {
            module: module.into(),
        }
    }

    pub(crate) fn external_module(owner: &[String], module: &[String]) -> Self {
        Self::ExternalModule {
            owner: owner.into(),
            module: module.into(),
        }
    }
}

pub(super) fn encode_type_path(
    encoder: &mut Encoder,
    path: &NamePath,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    let segments = path.segments().collect::<Vec<_>>();
    let resolved = match segments.as_slice() {
        [alias, name] => budget
            .resolved_aliases
            .get(*alias)
            .cloned()
            .map(|target| (target, *name)),
        _ => None,
    };
    let Some((target, name)) = resolved else {
        return encode_path(encoder, path, budget);
    };
    let target_segment_count = match &target {
        ResolvedAliasTarget::LocalModule { module } => module.len(),
        ResolvedAliasTarget::ExternalModule { owner, module } => {
            owner.len().checked_add(module.len()).ok_or_else(|| {
                source_identity_error("resolved type path segment count overflows usize")
            })?
        }
    };
    let segment_count = target_segment_count
        .checked_add(1)
        .ok_or_else(|| source_identity_error("resolved type path segment count overflows usize"))?;
    if segment_count > budget.limits.max_path_segments {
        return Err(source_identity_error(format!(
            "resolved type path has {segment_count} segments, exceeding the {} segment limit",
            budget.limits.max_path_segments
        )));
    }
    encoder.u32(RESOLVED_TYPE_PATH_DISCRIMINANT)?;
    match &target {
        ResolvedAliasTarget::LocalModule { module } => {
            encoder.u16(1)?;
            encode_target_segments(encoder, module, budget)?;
        }
        ResolvedAliasTarget::ExternalModule { owner, module } => {
            encoder.u16(2)?;
            encode_target_segments(encoder, owner, budget)?;
            encode_target_segments(encoder, module, budget)?;
        }
    }
    encode_name(encoder, name, budget)
}

fn encode_target_segments(
    encoder: &mut Encoder,
    segments: &[String],
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.u32(as_u32(segments.len(), "resolved target segment count")?)?;
    for segment in segments {
        encode_name(encoder, segment, budget)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{super::LocalSourceIdentity, ResolvedAliasTarget};

    #[test]
    fn alias_spelling_does_not_change_external_target_identity() {
        let source = |alias: &str| {
            eqiora_lang::parse(
                "alias.eqi",
                &format!(
                    "public component Side {{ public support body: volume(ambient_dimension = 2); public support wall: boundary(parent = body); public port p: conserving {alias}.Boundary over wall; }}"
                ),
            )
            .into_document()
            .expect("alias source")
        };
        let aliases = |alias: &str| {
            BTreeMap::from([(
                alias.to_owned(),
                ResolvedAliasTarget::external_module(
                    &["org.example".to_owned(), "mechanics".to_owned()],
                    &["main".to_owned()],
                ),
            )])
        };
        assert_eq!(
            LocalSourceIdentity::from_document_with_resolved_aliases(
                &source("short"),
                &aliases("short"),
            )
            .unwrap(),
            LocalSourceIdentity::from_document_with_resolved_aliases(
                &source("renamed"),
                &aliases("renamed"),
            )
            .unwrap(),
        );
    }

    #[test]
    fn admitted_targets_encode_kind_owner_and_module_without_collisions() {
        let document = eqiora_lang::parse(
            "alias.eqi",
            "model Main { instance value: dependency.Component; }",
        )
        .into_document()
        .expect("alias source");
        let identity = |target| {
            LocalSourceIdentity::from_document_with_resolved_aliases(
                &document,
                &BTreeMap::from([("dependency".to_owned(), target)]),
            )
            .expect("resolved source identity")
        };

        let owner_contains_old_separator = identity(ResolvedAliasTarget::external_module(
            &["owner".to_owned(), "module".to_owned()],
            &["part".to_owned()],
        ));
        let module_contains_old_separator = identity(ResolvedAliasTarget::external_module(
            &["owner".to_owned()],
            &["module".to_owned(), "part".to_owned()],
        ));
        let local_module = identity(ResolvedAliasTarget::local_module(&["part".to_owned()]));
        let external_same_module = identity(ResolvedAliasTarget::external_module(
            &["owner".to_owned()],
            &["part".to_owned()],
        ));
        let authored_alias_path = LocalSourceIdentity::from_document(&document)
            .expect("unresolved authored path identity");

        assert_ne!(owner_contains_old_separator, module_contains_old_separator);
        assert_ne!(module_contains_old_separator, local_module);
        assert_ne!(external_same_module, local_module);
        assert_ne!(external_same_module, authored_alias_path);
    }
}
