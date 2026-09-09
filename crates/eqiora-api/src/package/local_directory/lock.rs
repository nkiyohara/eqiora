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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequestPin {
    pub declaring: QualifiedName,
    pub declaring_version: ExactVersion,
    pub dependency: QualifiedName,
    pub request: VersionRequest,
    pub selected: ExactVersion,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProjectLock {
    schema: String,
    pub resolution: ResolutionRecordV1,
    pub git: Vec<GitPin>,
    pub requests: Vec<RequestPin>,
}

impl ProjectLock {
    pub fn versions(&self) -> BTreeMap<QualifiedName, ExactVersion> {
        self.resolution
            .nodes()
            .iter()
            .map(|node| {
                (
                    node.identity().name.clone(),
                    node.identity().version.clone(),
                )
            })
            .collect()
    }
    pub fn new(
        resolution: ResolutionRecordV1,
        mut git: Vec<GitPin>,
        mut requests: Vec<RequestPin>,
    ) -> Result<Self, PackagePreparationError> {
        git.sort();
        git.dedup();
        // Same-content mirrors can provide distinct commits under the same ref.
        // The complete sorted set is provenance, never a semantic version tie-break.
        for pin in &git {
            git::validate_revision(&pin.request)?;
            if let Some(repository) = &pin.repository {
                if !repository.starts_with("https://") {
                    return Err(git::error(
                        "Git lock locators must be explicit public HTTPS repositories",
                    ));
                }
                git::GitSource {
                    repository: repository.clone(),
                    rev: pin.request.clone(),
                }
                .validate()?;
            }
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
        requests.sort_by(|left, right| {
            (&left.declaring, &left.declaring_version, &left.dependency).cmp(&(
                &right.declaring,
                &right.declaring_version,
                &right.dependency,
            ))
        });
        if requests.len() != resolution.edges().len() {
            return Err(git::error(
                "authored requests must cover every exact selected edge",
            ));
        }
        let mut bindings = BTreeSet::new();
        for pin in &requests {
            if !bindings.insert((&pin.declaring, &pin.declaring_version, &pin.dependency))
                || !pin.request.matches(&pin.selected)
                || !resolution.edges().iter().any(|edge| {
                    edge.declaring().name == pin.declaring
                        && edge.declaring().version == pin.declaring_version
                        && edge.target().name == pin.dependency
                        && edge.target().version == pin.selected
                })
            {
                return Err(git::error(
                    "authored request does not bind one exact selected edge",
                ));
            }
        }
        Ok(Self {
            schema: "eqiora.project-lock.v2".to_owned(),
            resolution,
            git,
            requests,
        })
    }

    pub fn bytes(&self) -> Result<Vec<u8>, PackagePreparationError> {
        serde_json::to_vec(self).map_err(|_| git::error("cannot encode project lock"))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, PackagePreparationError> {
        let decoded: Self = serde_json::from_slice(bytes)
            .map_err(|_| git::error("invalid current project lock"))?;
        if decoded.schema != "eqiora.project-lock.v2" {
            return Err(git::error("unsupported project lock schema"));
        }
        // Validate the embedded semantic owner through its own decoder.
        let resolution = ResolutionRecordV1::from_json(&decoded.resolution.canonical_json()?)?;
        let result = Self::new(resolution, decoded.git, decoded.requests)?;
        if result.bytes()? != bytes {
            return Err(git::error("project lock is not canonical"));
        }
        Ok(result)
    }
}

pub(super) fn require_requests(
    expected: &[RequestPin],
    actual: &[RequestPin],
) -> Result<(), PackagePreparationError> {
    let map = |pins: &[RequestPin]| {
        pins.iter()
            .map(|pin| {
                (
                    (
                        pin.declaring.clone(),
                        pin.declaring_version.clone(),
                        pin.dependency.clone(),
                    ),
                    (pin.request.clone(), pin.selected.clone()),
                )
            })
            .collect::<BTreeMap<_, _>>()
    };
    if map(expected) != map(actual) {
        return Err(git::error(
            "authored requests differ from accepted eqiora.lock; explicitly update the lock",
        ));
    }
    Ok(())
}
