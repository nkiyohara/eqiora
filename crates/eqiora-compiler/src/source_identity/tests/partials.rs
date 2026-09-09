//! Explicit partial binding sets participate in canonical source identity.
use super::*;

#[test]
fn partial_identity_retains_selected_binding_and_unordered_holding_set() {
    let source = "model M() { parameter x:1=3; parameter y:1=5; parameter z:1=7; relation r { partial(x*x*y,wrt=x,holding=(y,z))=0; } }";
    let original = identity(source);
    assert_eq!(original, identity(&format(&document(source))));
    assert_eq!(
        original,
        identity(&source.replace("holding=(y,z)", "holding=(z,y)"))
    );
    for changed in [
        source.replace("wrt=x,holding=(y,z)", "wrt=y,holding=(x,z)"),
        source.replace("holding=(y,z)", "holding=(y)"),
        source.replace("partial(x*x*y,wrt=x,holding=(y,z))", "x*x*y"),
    ] {
        assert_ne!(original, identity(&changed));
    }
}
