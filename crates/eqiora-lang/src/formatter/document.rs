use core::fmt::Write;

use crate::Document;

pub(super) fn format_header(document: &Document, output: &mut String) -> usize {
    let mut count = 0;
    if let Some((module, _)) = document.module() {
        writeln!(output, "module {module};").expect("String writes cannot fail");
        count = 1;
    }
    for (module, alias, _) in document.imports() {
        writeln!(output, "import {module} as {alias};").expect("String writes cannot fail");
    }
    count + usize::from(document.imports().len() != 0)
}
