use core::fmt::Write;

use crate::Document;

pub(super) fn format_header(document: &Document, output: &mut String) -> usize {
    for (module, alias, _) in document.imports() {
        if module.segments().last() == Some(alias) {
            writeln!(output, "import {module};").expect("String writes cannot fail");
        } else {
            writeln!(output, "import {module} as {alias};").expect("String writes cannot fail");
        }
    }
    usize::from(document.imports().len() != 0)
}
