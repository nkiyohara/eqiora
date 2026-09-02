use core::fmt::Write;

use crate::Document;

pub(super) fn format_imports(document: &Document, output: &mut String) -> usize {
    for (module, alias, _) in document.imports() {
        writeln!(output, "import {module} as {alias};").expect("String writes cannot fail");
    }
    usize::from(document.imports().len() != 0)
}
