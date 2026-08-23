//! Narrow external Gmsh producer for the admitted exact-cylinder mesh path.

use std::ffi::{OsStr, OsString};
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use std::os::unix::process::CommandExt as _;

use eqiora::Diagnostic;
use eqiora::artifact::{
    AcceptedCircularHoleChordalRealizationV1, GeometryMeshCorrespondenceEnvelopeV1,
    SimplicialMeshEnvelopeV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::PlanarRegion;
use eqiora::io::gmsh::{GmshImportLimits, GmshSimplexImporter};
use eqiora::meshing::MeshQualityGate;
#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};

const GMSH_VERSION: &str = "4.15.2";
const GMSH_ENV: &str = "EQIORA_GMSH";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);

static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn generate(
    reference: &AcceptedCircularHoleChordalRealizationV1,
    quality_gate: MeshQualityGate,
) -> Result<AcceptedCircularHoleChordalRealizationV1, Diagnostic> {
    let executable = gmsh_executable()?;
    let scratch = ScratchDirectory::create()?;
    require_version(&executable, scratch.path())?;

    let geometry_path = scratch.path().join("mesh.geo");
    let mesh_path = scratch.path().join("mesh.msh");
    let region = reference.realized_geometry().region()?;
    fs::write(&geometry_path, geometry_script(&region)?).map_err(|error| {
        invalid_import(format!("cannot write the Gmsh geometry input: {error}"))
    })?;

    let limits = GmshImportLimits::default();
    let importer = GmshSimplexImporter::new(2, quality_gate, limits)?;
    let mut command = bounded_output_command(&executable, limits.max_bytes.saturating_add(1));
    command
        .arg("-2")
        .arg(&geometry_path)
        .arg("-o")
        .arg(&mesh_path)
        .args(["-format", "msh41", "-v", "2"])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = run_with_timeout(command, PROCESS_TIMEOUT)?;
    if !status.success() {
        return Err(invalid_import(format!(
            "Gmsh {GMSH_VERSION} failed with status {status}"
        )));
    }

    let bytes = read_bounded_output(
        &mesh_path,
        limits.max_bytes,
        "Gmsh input exceeds the configured byte limit",
        "Gmsh mesh output",
    )?;
    let mesh = importer.import_bytes(&bytes)?;
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(&mesh)?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(reference.realized_geometry(), &mesh)?;
    reference.bind_conforming_mesh(&mesh, &correspondence)
}

fn gmsh_executable() -> Result<OsString, Diagnostic> {
    match std::env::var_os(GMSH_ENV) {
        Some(value) if !value.is_empty() => Ok(value),
        Some(_) => Err(invalid_import(format!("{GMSH_ENV} must not be empty"))),
        None => Ok(OsString::from("gmsh")),
    }
}

fn require_version(executable: &OsStr, scratch: &Path) -> Result<(), Diagnostic> {
    let output_path = scratch.join("version.txt");
    let output = File::create(&output_path)
        .map_err(|error| invalid_import(format!("cannot create Gmsh version output: {error}")))?;
    let mut command = bounded_output_command(executable, 65);
    command
        .arg("--version")
        .stdout(Stdio::from(output))
        .stderr(Stdio::null());
    let status = run_with_timeout(command, VERSION_TIMEOUT)?;
    if !status.success() {
        return Err(invalid_import(format!(
            "Gmsh version check failed with status {}",
            status
        )));
    }
    let stdout = read_bounded_output(
        &output_path,
        64,
        "Gmsh version output exceeds 64 bytes",
        "Gmsh version output",
    )?;
    let version = std::str::from_utf8(&stdout)
        .map_err(|_| invalid_import("Gmsh version output must be UTF-8"))?
        .trim();
    if version != GMSH_VERSION {
        return Err(invalid_import(format!(
            "automatic meshing requires Gmsh {GMSH_VERSION}; found {version:?}"
        )));
    }
    Ok(())
}

fn read_bounded_output(
    path: &Path,
    max_bytes: usize,
    over_limit: &str,
    description: &str,
) -> Result<Vec<u8>, Diagnostic> {
    let length = fs::metadata(path)
        .map_err(|error| invalid_import(format!("cannot inspect {description}: {error}")))?
        .len();
    if length > max_bytes as u64 {
        return Err(invalid_import(over_limit));
    }
    let mut bytes = Vec::with_capacity(length as usize);
    File::open(path)
        .map_err(|error| invalid_import(format!("cannot read {description}: {error}")))?
        .take(max_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| invalid_import(format!("cannot read {description}: {error}")))?;
    if bytes.len() > max_bytes {
        return Err(invalid_import(over_limit));
    }
    Ok(bytes)
}

fn bounded_output_command(executable: &OsStr, max_bytes: usize) -> Command {
    #[cfg(unix)]
    {
        const FILE_LIMIT_BLOCK_BYTES: usize = 512;
        let blocks = max_bytes.div_ceil(FILE_LIMIT_BLOCK_BYTES);
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "ulimit -f \"$1\" || exit 126; shift; exec \"$@\"",
                "eqiora-gmsh",
            ])
            .arg(blocks.to_string())
            .arg(executable);
        command
    }
    #[cfg(not(unix))]
    {
        let _ = max_bytes;
        Command::new(executable)
    }
}

fn geometry_script(region: &PlanarRegion) -> Result<String, Diagnostic> {
    let [face] = region.faces() else {
        return Err(invalid_import(
            "automatic Gmsh meshing currently requires one planar face",
        ));
    };
    if face.holes().len() != 1 {
        return Err(invalid_import(
            "automatic Gmsh meshing currently requires one planar hole",
        ));
    }

    let mut script = String::from(
        "SetFactory(\"Built-in\");\n\
         General.NumThreads = 1;\n\
         Mesh.Algorithm = 6;\n\
         Mesh.ElementOrder = 1;\n\
         Mesh.Binary = 0;\n\
         Mesh.MshFileVersion = 4.1;\n\
         Mesh.RandomFactor = 0;\n\
         Mesh.SaveAll = 1;\n",
    );
    let point_order = face.holes()[0]
        .iter()
        .copied()
        .chain(face.outer().iter().copied())
        .collect::<Vec<_>>();
    if point_order.len() != region.vertices().len() {
        return Err(invalid_import(
            "automatic Gmsh meshing requires distinct boundary vertices",
        ));
    }
    let mut point_tag = vec![0_usize; region.vertices().len()];
    for (offset, vertex) in point_order.into_iter().enumerate() {
        if point_tag[vertex] != 0 {
            return Err(invalid_import(
                "automatic Gmsh meshing requires distinct boundary vertices",
            ));
        }
        let tag = offset + 1;
        point_tag[vertex] = tag;
        let [x, y] = region.vertices()[vertex];
        writeln!(script, "Point({tag}) = {{{x:?}, {y:?}, 0}};")
            .expect("writing to String cannot fail");
    }

    let mut next_line = 1_usize;
    let mut surface_loops = Vec::new();
    for (offset, vertices) in face
        .holes()
        .iter()
        .map(Vec::as_slice)
        .chain(std::iter::once(face.outer()))
        .enumerate()
    {
        let next_loop = offset + 1;
        let mut lines = Vec::with_capacity(vertices.len());
        for pair in vertices
            .iter()
            .copied()
            .zip(vertices.iter().copied().cycle().skip(1))
            .take(vertices.len())
        {
            writeln!(
                script,
                "Line({next_line}) = {{{}, {}}};",
                point_tag[pair.0], point_tag[pair.1]
            )
            .expect("writing to String cannot fail");
            lines.push(next_line);
            next_line += 1;
        }
        writeln!(
            script,
            "Curve Loop({next_loop}) = {{{}}};",
            comma_separated(&lines)
        )
        .expect("writing to String cannot fail");
        surface_loops.push(next_loop);
    }
    let outer_loop = surface_loops.pop().expect("the outer loop was generated");
    let mut plane_loops = vec![outer_loop];
    plane_loops.extend(surface_loops);
    writeln!(
        script,
        "Plane Surface(1) = {{{}}};",
        comma_separated(&plane_loops)
    )
    .expect("writing to String cannot fail");
    Ok(script)
}

fn comma_separated(values: &[usize]) -> String {
    values
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn run_with_timeout(mut command: Command, timeout: Duration) -> Result<ExitStatus, Diagnostic> {
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command
        .spawn()
        .map_err(|error| invalid_import(format!("cannot launch Gmsh: {error}")))?;
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(status) = child
            .try_wait()
            .map_err(|error| invalid_import(format!("cannot wait for Gmsh: {error}")))?
        {
            kill_descendants(&child);
            return Ok(status);
        }
        if Instant::now() >= deadline {
            terminate(&mut child);
            return Err(invalid_import(
                "Gmsh exceeded the 30 second execution limit",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
}

#[cfg(unix)]
fn kill_descendants(child: &Child) {
    if let Ok(raw_pid) = i32::try_from(child.id())
        && let Some(pid) = Pid::from_raw(raw_pid)
    {
        let _ = kill_process_group(pid, Signal::KILL);
    }
}

#[cfg(not(unix))]
fn kill_descendants(_child: &Child) {}

fn terminate(child: &mut Child) {
    kill_descendants(child);
    let _ = child.kill();
    let _ = child.wait();
}

struct ScratchDirectory {
    path: PathBuf,
}

impl ScratchDirectory {
    fn create() -> Result<Self, Diagnostic> {
        let parent = std::env::temp_dir();
        for _ in 0..16 {
            let nonce = SCRATCH_NONCE.fetch_add(1, Ordering::Relaxed);
            let clock = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path = parent.join(format!(
                "eqiora-gmsh-{}-{clock}-{nonce}",
                std::process::id()
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Ok(Self { path }),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(invalid_import(format!(
                        "cannot create Gmsh scratch directory: {error}"
                    )));
                }
            }
        }
        Err(invalid_import(
            "cannot allocate a unique Gmsh scratch directory",
        ))
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn invalid_import(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_MESH_IMPORT, message)
}
