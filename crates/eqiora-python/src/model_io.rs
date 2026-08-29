//! Bounded local-file boundary for canonical Python Model artifacts.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use eqiora::Diagnostic;
use eqiora::api::ModelDocument;
use eqiora::artifact::{ModelDecoderLimits, ModelEnvelope};
use eqiora::diagnostic::codes;

pub(crate) const MODEL_FILE_EXTENSION: &str = "eqmodel";

static STAGING_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) enum DecodedModel {
    Document(Box<ModelDocument>),
    Deferred(ModelEnvelope),
}

pub(crate) fn decode_model(bytes: &[u8]) -> Result<DecodedModel, Vec<Diagnostic>> {
    let artifact = ModelEnvelope::from_json(bytes, ModelDecoderLimits::default())
        .map_err(|diagnostic| vec![diagnostic])?;
    let canonical = artifact
        .canonical_json()
        .map_err(|diagnostic| vec![diagnostic])?;
    if canonical != bytes {
        return Err(vec![invalid_model_artifact(
            "Model artifact bytes are not the exact canonical encoding",
        )]);
    }
    if artifact
        .requires_geometry_admission()
        .map_err(|diagnostic| vec![diagnostic])?
    {
        Ok(DecodedModel::Deferred(artifact))
    } else {
        ModelDocument::replay(bytes).map(|document| DecodedModel::Document(Box::new(document)))
    }
}

pub(crate) fn validate_model_path(path: &Path) -> Result<(), Diagnostic> {
    if path
        .extension()
        .is_none_or(|suffix| suffix != MODEL_FILE_EXTENSION)
    {
        return Err(invalid_model_artifact(format!(
            "compiled Model paths must use the .{MODEL_FILE_EXTENSION} suffix"
        )));
    }
    Ok(())
}

pub(crate) fn read_model_bytes(path: &Path) -> Result<Vec<u8>, Diagnostic> {
    validate_model_path(path)?;
    let link_metadata =
        fs::symlink_metadata(path).map_err(|error| model_io_error("inspect", path, &error))?;
    if !link_metadata.file_type().is_file() {
        return Err(invalid_model_artifact(format!(
            "compiled Model input {:?} is not a regular file",
            path.display()
        )));
    }

    let file = File::open(path).map_err(|error| model_io_error("open", path, &error))?;
    let metadata = file
        .metadata()
        .map_err(|error| model_io_error("inspect opened", path, &error))?;
    if !metadata.file_type().is_file() {
        return Err(invalid_model_artifact(format!(
            "compiled Model input {:?} did not remain a regular file",
            path.display()
        )));
    }

    let max_bytes = ModelDecoderLimits::default().json.max_bytes;
    if metadata.len() > max_bytes as u64 {
        return Err(invalid_model_artifact(format!(
            "compiled Model input has {} bytes, exceeding the {max_bytes} byte decoder limit",
            metadata.len()
        )));
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take((max_bytes + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| model_io_error("read", path, &error))?;
    if bytes.len() > max_bytes {
        return Err(invalid_model_artifact(format!(
            "compiled Model input exceeds the {max_bytes} byte decoder limit"
        )));
    }
    Ok(bytes)
}

pub(crate) fn write_model_bytes(path: &Path, bytes: &[u8]) -> Result<(), Diagnostic> {
    validate_model_path(path)?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(invalid_model_artifact(format!(
                "compiled Model output {:?} is not a regular file",
                path.display()
            )));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(model_io_error("inspect output", path, &error)),
    }

    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let (staging_path, mut staging) = create_staging_file(parent, path)?;
    let mut cleanup = StagingCleanup::new(staging_path.clone());
    staging
        .write_all(bytes)
        .and_then(|()| staging.sync_all())
        .map_err(|error| model_io_error("write", &staging_path, &error))?;
    drop(staging);
    fs::rename(&staging_path, path).map_err(|error| model_io_error("publish", path, &error))?;
    cleanup.published = true;
    Ok(())
}

fn create_staging_file(parent: &Path, target: &Path) -> Result<(PathBuf, File), Diagnostic> {
    for _ in 0..128 {
        let sequence = STAGING_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let staging_path = parent.join(format!(
            ".eqiora-model-{}-{sequence}.tmp",
            std::process::id()
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&staging_path)
        {
            Ok(file) => return Ok((staging_path, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(model_io_error("stage", target, &error)),
        }
    }
    Err(invalid_model_artifact(format!(
        "could not allocate a staging file beside compiled Model output {:?}",
        target.display()
    )))
}

struct StagingCleanup {
    path: PathBuf,
    published: bool,
}

impl StagingCleanup {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            published: false,
        }
    }
}

impl Drop for StagingCleanup {
    fn drop(&mut self) {
        if !self.published {
            let _ = fs::remove_file(&self.path);
        }
    }
}

fn model_io_error(action: &str, path: &Path, error: &std::io::Error) -> Diagnostic {
    invalid_model_artifact(format!(
        "could not {action} compiled Model path {:?}: {error}",
        path.display()
    ))
}

fn invalid_model_artifact(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
