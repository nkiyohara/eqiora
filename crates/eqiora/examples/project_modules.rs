use eqiora::api::ModelDocument;

const MAIN: &str = include_str!("../../../examples/modules/resistor-project/src/main.eqi");
const RESISTOR: &str =
    include_str!("../../../examples/modules/resistor-project/src/library/resistor.eqi");

fn main() {
    let document = ModelDocument::compile_project_sources(
        "models.main",
        [
            ("src/main.eqi", MAIN),
            ("src/library/resistor.eqi", RESISTOR),
        ],
        "Main",
    )
    .expect("the checked-in project source closure compiles");

    println!("{}", document.digest().expect("Model digest"));
}
