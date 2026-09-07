//! Current project provenance envelope; semantic resolution retains its own wire.

use super::*;

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitPin {
    pub declaring: QualifiedName,
    pub declaring_version: ExactVersion,
    pub dependency: QualifiedName,
    pub version: ExactVersion,
    pub request: String,
    pub commit: String,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectLock {
    schema: String,
    pub resolution: ResolutionRecordV1,
    pub git: Vec<GitPin>,
}

impl ProjectLock {
    pub fn new(
        resolution: ResolutionRecordV1,
        mut git: Vec<GitPin>,
    ) -> Result<Self, PackagePreparationError> {
        git.sort();
        git.dedup();
        for pair in git.windows(2) {
            if (
                &pair[0].declaring,
                &pair[0].declaring_version,
                &pair[0].dependency,
            ) == (
                &pair[1].declaring,
                &pair[1].declaring_version,
                &pair[1].dependency,
            ) {
                return Err(git::error("duplicate Git selection"));
            }
        }
        for pin in &git {
            git::validate_revision(&pin.request)?;
            if !git::is_commit(&pin.commit)
                || !resolution.edges().iter().any(|edge| {
                    edge.declaring().name == pin.declaring
                        && edge.declaring().version == pin.declaring_version
                        && edge.target().name == pin.dependency
                        && edge.target().version == pin.version
                })
            {
                return Err(git::error(
                    "Git selection does not bind an exact dependency edge",
                ));
            }
        }
        Ok(Self {
            schema: "eqiora.project-lock.v1".to_owned(),
            resolution,
            git,
        })
    }

    pub fn bytes(&self) -> Result<Vec<u8>, PackagePreparationError> {
        serde_json::to_vec(self).map_err(|_| git::error("cannot encode project lock"))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PackagePreparationError> {
        let decoded: Self = serde_json::from_slice(bytes)
            .map_err(|_| git::error("invalid current project lock"))?;
        if decoded.schema != "eqiora.project-lock.v1" {
            return Err(git::error("unsupported project lock schema"));
        }
        // Validate the embedded semantic owner through its own decoder.
        let resolution = ResolutionRecordV1::from_json(&decoded.resolution.canonical_json()?)?;
        let result = Self::new(resolution, decoded.git)?;
        if result.bytes()? != bytes {
            return Err(git::error("project lock is not canonical"));
        }
        Ok(result)
    }
}
