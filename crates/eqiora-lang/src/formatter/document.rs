use core::fmt::Write;

use crate::Document;

pub(super) fn format_header(
    document: &Document,
    output: &mut crate::formatter::comments::Output,
) -> usize {
    for import in &document.imports {
        output.begin(&import.comments);
        let (module, alias) = (&import.module, &import.alias);
        if module.segments().last() == Some(alias) {
            writeln!(output, "import {module};").expect("String writes cannot fail");
        } else {
            writeln!(output, "import {module} as {alias};").expect("String writes cannot fail");
        }
        output.end();
    }
    usize::from(document.imports().len() != 0)
}
