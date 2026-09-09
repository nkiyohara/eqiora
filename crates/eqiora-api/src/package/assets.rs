//! Exact package-owned data assets admitted before compiler analysis.
use eqiora_artifact::{ResolvedArrayDecoderLimits, ResolvedArrayV1};
use eqiora_core::{Diagnostic, diagnostic::codes};
use eqiora_package::{BundleRoleV1, SourceFileV1};
use eqiora_schema::resolved_array::ResolvedF64Array;
use std::{collections::BTreeMap, sync::Arc};

pub(super) fn arrays(
    files: &[SourceFileV1],
) -> Result<Arc<BTreeMap<String, ResolvedF64Array>>, Diagnostic> {
    let mut arrays = BTreeMap::new();
    for file in files
        .iter()
        .filter(|file| file.role() == BundleRoleV1::ResolvedArray)
    {
        let name = file
            .path()
            .as_str()
            .strip_prefix("data/")
            .and_then(|path| path.strip_suffix(".json"))
            .ok_or_else(|| error("resolved array path must use data/<NamePath segments>.json"))?;
        if name.split('/').any(|part| part.contains('.')) {
            return Err(error("resolved array path segments must be identifiers"));
        }
        let name = name.replace('/', ".");
        eqiora_package::QualifiedName::parse(&name)
            .map_err(|problem| error(problem.to_string()))?;
        let array =
            ResolvedArrayV1::from_json(file.bytes(), ResolvedArrayDecoderLimits::default())?;
        let array = array
            .resolved_f64()
            .ok_or_else(|| error("property data assets require finite binary64 resolved arrays"))?
            .clone();
        if arrays.insert(name, array).is_some() {
            return Err(error("duplicate exact package array asset name"));
        }
    }
    Ok(Arc::new(arrays))
}
fn error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::LANGUAGE_LOWERING_ERROR, message)
}

type Documents = BTreeMap<String, Arc<[u8]>>;

/// Documentation entries become typed references only under the fixed docs/ mapping.
pub(super) fn documents(files: &[SourceFileV1]) -> Result<Arc<Documents>, Diagnostic> {
    let mut documents = BTreeMap::new();
    for file in files
        .iter()
        .filter(|file| file.role() == BundleRoleV1::Documentation)
    {
        let Some(name) = file
            .path()
            .as_str()
            .strip_prefix("docs/")
            .and_then(|path| path.strip_suffix(".md"))
        else {
            continue;
        };
        if name.split('/').any(|part| part.contains('.')) {
            return Err(error(
                "documentation asset path segments must be identifiers",
            ));
        }
        let name = name.replace('/', ".");
        eqiora_package::QualifiedName::parse(&name)
            .map_err(|problem| error(problem.to_string()))?;
        std::str::from_utf8(file.bytes())
            .map_err(|problem| error(format!("documentation asset is not UTF-8: {problem}")))?;
        if documents.insert(name, Arc::from(file.bytes())).is_some() {
            return Err(error("duplicate exact package documentation asset name"));
        }
    }
    Ok(Arc::new(documents))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_package::NormalizedRelativePath;
    fn file(path: &str, role: BundleRoleV1, bytes: Vec<u8>) -> SourceFileV1 {
        SourceFileV1::new(NormalizedRelativePath::parse(path).unwrap(), role, bytes)
    }
    #[test]
    fn exact_typed_names_retain_verified_array_payload() {
        let array = ResolvedArrayV1::from_f64(vec![2, 2], vec![0., 1., 2., 3.]).unwrap();
        let files = [file(
            "data/curves/Cp.json",
            BundleRoleV1::ResolvedArray,
            array.canonical_json().unwrap(),
        )];
        let assets = arrays(&files).unwrap();
        assert_eq!(
            assets["curves.Cp"].digest().unwrap(),
            array.digest().unwrap().sha256_bytes()
        );
        assert_eq!(assets["curves.Cp"].values(), &[0., 1., 2., 3.]);
        assert!(!assets.contains_key("Cp"));
        assert!(!assets.contains_key("data/curves/Cp.json"));
        assert!(arrays(&[files[0].clone(), files[0].clone()]).is_err());
        assert!(
            arrays(&[file(
                "data/curves/Cp.json",
                BundleRoleV1::Documentation,
                vec![]
            )])
            .unwrap()
            .is_empty()
        );
    }
    #[test]
    fn malformed_payload_path_scalar_and_zero_are_rejected() {
        let unsigned = ResolvedArrayV1::from_u64(vec![2], vec![1, 2]).unwrap();
        assert!(
            arrays(&[file(
                "data/Cp.json",
                BundleRoleV1::ResolvedArray,
                unsigned.canonical_json().unwrap()
            )])
            .is_err()
        );
        for path in [
            "outside/Cp.json",
            "data/has.dot.json",
            "data/invalid-name.json",
        ] {
            assert!(arrays(&[file(path, BundleRoleV1::ResolvedArray, vec![])]).is_err());
        }
        for payload in [b"not JSON".as_slice(), br#"{"schema":"eqiora.resolved-array/v1","encoding":"eqiora.canonical-json/v1","scalar":"f64","shape":[1],"values":[-0.0]}"#] {
            assert!(arrays(&[file("data/Cp.json", BundleRoleV1::ResolvedArray, payload.to_vec())]).is_err());
        }
    }
    #[test]
    fn attribution_payloads_are_exact_utf8_typed_assets() {
        let payload = b"exact citation\n";
        let docs = documents(&[
            file(
                "docs/source/paper.md",
                BundleRoleV1::Documentation,
                payload.to_vec(),
            ),
            file(
                "README.md",
                BundleRoleV1::Documentation,
                b"ordinary".to_vec(),
            ),
        ])
        .unwrap();
        assert_eq!(docs.len(), 1);
        assert_eq!(docs["source.paper"].as_ref(), payload);
        assert!(!docs.contains_key("paper"));
        assert!(
            documents(&[file(
                "docs/invalid-name.md",
                BundleRoleV1::Documentation,
                vec![]
            )])
            .is_err()
        );
        assert!(
            documents(&[file(
                "docs/paper.md",
                BundleRoleV1::Documentation,
                vec![255]
            )])
            .is_err()
        );
        let duplicate = file("docs/paper.md", BundleRoleV1::Documentation, vec![]);
        assert!(documents(&[duplicate.clone(), duplicate]).is_err());
        assert!(
            documents(&[file("docs/paper.md", BundleRoleV1::ModelSource, vec![])])
                .unwrap()
                .is_empty()
        );
    }
}
