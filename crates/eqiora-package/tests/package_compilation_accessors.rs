use eqiora_package::{
    CompilationToolchainV1, ExactVersion, PackageCompilationRecordV1, QualifiedName,
};

const ACCEPTED_COMPILATION: &[u8] = include_bytes!(
    "../../../verify/packages/typed-execution-lineage/expected/historical-alpha1-compilation.json"
);

fn canonical_fixture(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\n").unwrap_or(bytes)
}

#[test]
fn versioned_compilation_toolchain_fields_are_typed_read_only_accessors() {
    let record = PackageCompilationRecordV1::from_json(canonical_fixture(ACCEPTED_COMPILATION))
        .expect("accepted package-compilation record");

    let record_accessor: for<'a> fn(&'a PackageCompilationRecordV1) -> &'a CompilationToolchainV1 =
        PackageCompilationRecordV1::toolchain;
    let compiler_accessor: for<'a> fn(&'a CompilationToolchainV1) -> &'a QualifiedName =
        CompilationToolchainV1::compiler;
    let compiler_version_accessor: for<'a> fn(&'a CompilationToolchainV1) -> &'a ExactVersion =
        CompilationToolchainV1::compiler_version;
    let semantic_version_accessor: fn(&CompilationToolchainV1) -> u32 =
        CompilationToolchainV1::semantic_canonicalization_version;
    let source_version_accessor: fn(&CompilationToolchainV1) -> u32 =
        CompilationToolchainV1::source_bundle_version;
    let resolution_version_accessor: fn(&CompilationToolchainV1) -> u32 =
        CompilationToolchainV1::resolution_version;

    let toolchain = record_accessor(&record);
    assert_eq!(compiler_accessor(toolchain).as_str(), "Eqiora.Compiler");
    assert_eq!(
        compiler_version_accessor(toolchain).as_str(),
        "0.1.0-alpha.1"
    );
    assert_eq!(semantic_version_accessor(toolchain), 1);
    assert_eq!(source_version_accessor(toolchain), 1);
    assert_eq!(resolution_version_accessor(toolchain), 1);

    let replay = PackageCompilationRecordV1::from_json(
        &record
            .canonical_json()
            .expect("canonical compilation bytes"),
    )
    .expect("replayed compilation record");
    assert_eq!(replay, record);
    assert_eq!(
        replay.digest().expect("replayed compilation identity"),
        record.digest().expect("accepted compilation identity")
    );
}
