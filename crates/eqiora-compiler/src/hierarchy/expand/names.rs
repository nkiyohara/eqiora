use eqiora_schema::kernel::BoundarySide;

use crate::identity::{ElaborationIdentityLimits, FullElaborationIdentity, InstancePath};
use eqiora_core::Diagnostic;

pub(super) fn internal_name(identity: FullElaborationIdentity) -> String {
    format!("e{identity}")
}

pub(super) fn display_child(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}.{child}")
    }
}

pub(super) fn boundary_family_display(
    parent: &str,
    family: &str,
    axis: usize,
    side: BoundarySide,
) -> String {
    let side = match side {
        BoundarySide::Lower => "lower",
        BoundarySide::Upper => "upper",
    };
    format!("{}[axis={axis},side={side}]", display_child(parent, family))
}

pub(super) fn child_instance_path(
    parent: &InstancePath,
    child: &str,
    limits: ElaborationIdentityLimits,
) -> Result<InstancePath, Diagnostic> {
    InstancePath::with_limits(
        parent
            .segments()
            .iter()
            .map(String::as_str)
            .chain(core::iter::once(child)),
        limits,
    )
}
