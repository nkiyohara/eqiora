//! Explicit, contained Git source acquisition. Git never participates in compilation/reopening.

#[cfg(target_os = "linux")]
mod transport;

use super::*;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GitSource {
    pub repository: String,
    pub rev: String,
}

pub(super) fn error(message: &str) -> PackagePreparationError {
    PackagePreparationError::LocalDirectoryGraph(message.to_owned())
}

pub(super) fn is_commit(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn validate_revision(rev: &str) -> Result<(), PackagePreparationError> {
    if is_commit(rev) {
        return Ok(());
    }
    let name = rev
        .strip_prefix("refs/heads/")
        .or_else(|| rev.strip_prefix("refs/tags/"));
    if name.is_none_or(|name| name.is_empty())
        || rev.len() > 1024
        || rev.contains("..")
        || rev.contains("@{")
        || rev.ends_with('.')
        || rev
            .split('/')
            .any(|part| part.is_empty() || part.starts_with('.') || part.ends_with(".lock"))
        || rev
            .bytes()
            .any(|byte| byte <= b' ' || byte >= 127 || b"~^:?*[\\".contains(&byte))
    {
        return Err(error(
            "Git revision must be a lowercase full SHA-1 commit or explicit refs/heads/name or refs/tags/name",
        ));
    }
    Ok(())
}

impl GitSource {
    pub(super) fn matches_pin_transport(&self, pin: &lock::GitPin) -> bool {
        !self.repository.starts_with("https://")
            || pin.repository.as_deref() == Some(self.repository.as_str())
    }

    pub fn validate(&self) -> Result<(), PackagePreparationError> {
        validate_revision(&self.rev)?;
        let value = &self.repository;
        if value.is_empty()
            || value.len() > 4096
            || value.chars().any(char::is_control)
            || value.contains(['@', '?', '#', '\\', '%'])
        {
            return Err(error(
                "Git repository must be a public HTTPS URL without credentials/query/fragment or an explicit local path",
            ));
        }
        if let Some(remote) = value.strip_prefix("https://") {
            let (host, path) = remote
                .split_once('/')
                .ok_or_else(|| error("Git HTTPS repository requires a host and path"))?;
            if host.is_empty()
                || path.is_empty()
                || host.contains(':')
                || !host
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
            {
                return Err(error("invalid public HTTPS Git repository"));
            }
        } else if value.contains(':')
            || !(Path::new(value).is_absolute()
                || value.starts_with("./")
                || value.starts_with("../"))
        {
            return Err(error(
                "Git source supports public HTTPS or explicit absolute/./../ local repository paths only",
            ));
        }
        Ok(())
    }
}

impl PackagedModelDocument {
    /// Add an exact package from a Git commit or explicitly named ref.
    ///
    /// # Errors
    /// Rejects invalid requests, unavailable Linux containment, unsafe/oversized content,
    /// missing closure, preparation or atomic publication failure.
    pub fn add_git_package_dependency_v1(
        project_root: impl Into<PathBuf>,
        store_root: impl Into<PathBuf>,
        name: &str,
        version: &str,
        repository: &str,
        revision: &str,
    ) -> Result<ResolutionRecordV1, PackagePreparationError> {
        let source = GitSource {
            repository: repository.to_owned(),
            rev: revision.to_owned(),
        };
        source.validate()?;
        available()?;
        update_local_package_project(project_root.into(), store_root.into(), |manifest| {
            manifest.dependencies.insert(
                name.to_owned(),
                LocalProjectDependency {
                    version: version.to_owned(),
                    sources: vec![LocalDependencySource {
                        path: None,
                        bundled: false,
                        git: Some(source),
                    }],
                },
            );
            Ok(true)
        })
    }
}

fn available() -> Result<(), PackagePreparationError> {
    #[cfg(target_os = "linux")]
    {
        transport::available()
    }
    #[cfg(not(target_os = "linux"))]
    {
        Err(error(
            "Git fetch requires the supported Linux Git/prlimit containment environment",
        ))
    }
}

pub(super) fn freeze(
    source: &GitSource,
    locked: Option<&str>,
    declaring_path: &Path,
    parent: &LocalProjectOverrides,
    inventory: &mut inventory::Frozen,
) -> Result<(PackageKey, String), PackagePreparationError> {
    available()?;
    #[cfg(target_os = "linux")]
    {
        let mut child = child_context(source, parent)?;
        let fetched = transport::fetch(source, locked, declaring_path)?;
        let key = load_fetched(&fetched, &mut child, inventory)?;
        Ok((key, fetched.commit.clone()))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (source, locked, declaring_path, parent, inventory);
        unreachable!("available rejects unsupported platform")
    }
}

/// Try only immutable commits already belonging to the accepted exact edge.
/// Missing commits may be found in another explicitly configured mirror; once
/// acquired, rejected objects, source trees and declarations remain fatal.
pub(super) fn materialize(
    sources: &[GitSource],
    pins: &[&lock::GitPin],
    declaring_path: &Path,
    parent: &LocalProjectOverrides,
    inventory: &mut inventory::Frozen,
) -> Result<PackageKey, PackagePreparationError> {
    available()?;
    #[cfg(target_os = "linux")]
    {
        let mut sources = sources.iter().collect::<Vec<_>>();
        sources.sort_by(|left, right| {
            (&left.repository, &left.rev).cmp(&(&right.repository, &right.rev))
        });
        for pin in pins {
            for source in &sources {
                if !source.matches_pin_transport(pin) {
                    continue;
                }
                let mut child = child_context(source, parent)?;
                if let Some(fetched) =
                    transport::candidate(source, Some(&pin.commit), declaring_path)?
                {
                    return load_fetched(&fetched, &mut child, inventory);
                }
            }
        }
        Err(error(
            "cannot materialize an accepted immutable Git commit from explicit sources",
        ))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (sources, pins, declaring_path, parent, inventory);
        unreachable!("available rejects unsupported platform")
    }
}

#[cfg(target_os = "linux")]
fn child_context(
    source: &GitSource,
    parent: &LocalProjectOverrides,
) -> Result<LocalProjectOverrides, PackagePreparationError> {
    if parent.git_stack.len() >= 8 {
        return Err(error("Git dependency nesting exceeds 8 repositories"));
    }
    if parent
        .git_count
        .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        >= 16
    {
        return Err(error("project Git acquisition exceeds 16 repositories"));
    }
    let mut stack = parent.git_stack.clone();
    if !stack.insert(format!("{}\n{}", source.repository, source.rev)) {
        return Err(error("cyclic Git dependency request"));
    }
    Ok(LocalProjectOverrides {
        allow_git: true,
        confined: true,
        git_stack: stack,
        git_count: parent.git_count.clone(),
        locked_git: parent.locked_git.clone(),
        locked_versions: parent.locked_versions.clone(),
        locked_requests: parent.locked_requests.clone(),
        ..Default::default()
    })
}

#[cfg(target_os = "linux")]
fn load_fetched(
    fetched: &transport::Fetched,
    child: &mut LocalProjectOverrides,
    inventory: &mut inventory::Frozen,
) -> Result<PackageKey, PackagePreparationError> {
    let directory = open_project_root(&fetched.sources)?;
    inventory::load(
        &directory,
        &fetched.sources,
        PathBuf::new(),
        0,
        child,
        inventory,
    )
    .map_err(|_| error("fetched Git package or dependency closure is invalid"))
}
