mod command;
mod terminal;

use eqiora::api::ModelDocument;

#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use command::contained_run as run_for_oracle;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use command::read_bounded as read_bounded_for_oracle;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use command::{OraclePanicPoint, OracleReadError};
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use terminal::render_diagnostics as render_for_oracle;
#[cfg(test)]
#[allow(unused_imports)]
pub(crate) use terminal::severity_label as severity_label_for_oracle;

#[cfg_attr(test, allow(dead_code))]
fn main() {
    let outcome = command::contained_run(
        command::process_args,
        ModelDocument::compile,
        command::OraclePanicPoint::None,
    );
    std::process::exit(terminal::commit(outcome));
}
