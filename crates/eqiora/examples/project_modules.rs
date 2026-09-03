use eqiora::api::ModelDocument;

#[cfg(not(feature = "package-filesystem"))]
const MAIN: &str = include_str!("../../../examples/modules/resistor-project/src/main.eqi");
#[cfg(not(feature = "package-filesystem"))]
const RESISTOR: &str =
    include_str!("../../../examples/modules/resistor-project/src/library/resistor.eqi");

#[cfg(not(feature = "package-filesystem"))]
fn compile() -> ModelDocument {
    ModelDocument::compile_project_sources(
        "models.main",
        [
            ("src/main.eqi", MAIN),
            ("src/library/resistor.eqi", RESISTOR),
        ],
        "Main",
    )
    .expect("the checked-in project source closure compiles")
}

#[cfg(feature = "package-filesystem")]
fn compile() -> ModelDocument {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/modules/resistor-project");
    ModelDocument::compile_project_directory(root, "models.main", "Main")
        .expect("the discovered project source closure compiles")
}

fn main() {
    let document = compile();

    println!("{}", document.digest().expect("Model digest"));
}
