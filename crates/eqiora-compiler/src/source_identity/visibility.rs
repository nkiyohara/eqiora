use eqiora_core::Diagnostic;
use eqiora_lang::VisibilitySyntax;

use super::Encoder;

pub(super) fn encode_visibility(
    encoder: &mut Encoder,
    visibility: VisibilitySyntax,
) -> Result<(), Diagnostic> {
    encoder.u8(match visibility {
        VisibilitySyntax::Private => 1,
        VisibilitySyntax::Public => 2,
    })
}
