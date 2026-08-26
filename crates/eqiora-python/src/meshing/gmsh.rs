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
    AcceptedCircularHoleChordalRealizationV1, ExternalAdapterIdentityV1, ExternalImportManifestV1,
    ExternalImportObservationV1, ExternalImportSelectionV1, ExternalImportSourceV1,
    GeometryMeshCorrespondenceEnvelopeV1, ResolvedArrayV1, ResolvedImportArrayV1,
    SelectedSourceEntityV1, SimplicialMeshEnvelopeV1, StructuralSelectorV1,
};
use eqiora::diagnostic::codes;
use eqiora::geometry::CanonicalGeometryV1;
use eqiora::io::gmsh::{GmshImportLimits, GmshSimplexImporter};
use eqiora::meshing::{MeshEntity, MeshQualityGate, MeshTopology};
#[cfg(unix)]
use rustix::process::{Pid, Signal, kill_process_group};

const GMSH_VERSION: &str = "4.15.2";
const GMSH_ENV: &str = "EQIORA_GMSH";
const VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const PROCESS_TIMEOUT: Duration = Duration::from_secs(30);
const GMSH_ADAPTER_ID: &str = "eqiora.gmsh";
// Adapter-relative manifest selectors: the admitted whole MSH mesh, followed
// by its normalized Nodes geometry and Elements topology observations.
const MSH_MESH_SELECTOR: &[u32] = &[0];
const MSH_NODES_SELECTOR: &[u32] = &[1];
const MSH_ELEMENTS_SELECTOR: &[u32] = &[2];

static SCRATCH_NONCE: AtomicU64 = AtomicU64::new(0);

pub(super) struct ImportedGmshMesh {
    pub(super) accepted: AcceptedCircularHoleChordalRealizationV1,
    pub(super) manifest: ExternalImportManifestV1,
}

pub(super) struct GeneratedGmshMesh {
    pub(super) mesh: SimplicialMeshEnvelopeV1,
    pub(super) correspondence: GeometryMeshCorrespondenceEnvelopeV1,
    pub(super) edge_facets: [Vec<usize>; 5],
    pub(super) face_cells: Vec<usize>,
    pub(super) element_blocks: Vec<eqiora::io::gmsh::GmshElementBlock>,
    pub(super) source_edge_by_tag: std::collections::BTreeMap<u32, usize>,
    pub(super) source_face_by_tag: std::collections::BTreeMap<u32, usize>,
}

pub(super) fn import(
    source: &[u8],
    reference: &AcceptedCircularHoleChordalRealizationV1,
    quality_gate: MeshQualityGate,
) -> Result<ImportedGmshMesh, Diagnostic> {
    let importer = GmshSimplexImporter::new(2, quality_gate, GmshImportLimits::default())?;
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(&importer.import_bytes(source)?)?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_region(reference.realized_geometry(), &mesh)?;
    let accepted = reference.bind_conforming_mesh(&mesh, &correspondence)?;
    let observation = observation(source, &mesh)?;
    let selection = ExternalImportSelectionV1::new(
        SelectedSourceEntityV1::new(
            StructuralSelectorV1::new(MSH_MESH_SELECTOR.to_vec()),
            Some("MSH 4.1 mesh".to_owned()),
        )?,
        Vec::new(),
    )?;
    let manifest = ExternalImportManifestV1::from_observation(
        ExternalAdapterIdentityV1::new(GMSH_ADAPTER_ID, eqiora::VERSION)?,
        Vec::new(),
        selection,
        &observation,
        &mesh,
        &[],
    )?;
    Ok(ImportedGmshMesh { accepted, manifest })
}

fn observation(
    source: &[u8],
    mesh: &SimplicialMeshEnvelopeV1,
) -> Result<ExternalImportObservationV1, Diagnostic> {
    let native = mesh.mesh();
    let vertex_count = u64::try_from(native.vertices().len())
        .map_err(|_| invalid_import("Gmsh vertex count exceeds portable u64"))?;
    let dimension = u64::try_from(mesh.dimension())
        .map_err(|_| invalid_import("Gmsh dimension exceeds portable u64"))?;
    let cell_count = u64::try_from(native.cells().len())
        .map_err(|_| invalid_import("Gmsh cell count exceeds portable u64"))?;
    let cell_width = native.cells().first().map_or(0, Vec::len);
    let cell_width = u64::try_from(cell_width)
        .map_err(|_| invalid_import("Gmsh cell width exceeds portable u64"))?;
    let coordinates = native
        .vertices()
        .iter()
        .flat_map(|coordinate| coordinate.iter().copied())
        .collect();
    let topology = native
        .cells()
        .iter()
        .flatten()
        .map(|&index| {
            u64::try_from(index)
                .map_err(|_| invalid_import("Gmsh vertex index exceeds portable u64"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    ExternalImportObservationV1::new(
        ExternalImportSourceV1::metadata_document(source.to_vec(), None)?,
        Vec::new(),
        ResolvedImportArrayV1::new(
            0,
            StructuralSelectorV1::new(MSH_NODES_SELECTOR.to_vec()),
            Some("Nodes".to_owned()),
            ResolvedArrayV1::from_f64(vec![vertex_count, dimension], coordinates)?,
        )?,
        ResolvedImportArrayV1::new(
            0,
            StructuralSelectorV1::new(MSH_ELEMENTS_SELECTOR.to_vec()),
            Some("Elements".to_owned()),
            ResolvedArrayV1::from_u64(vec![cell_count, cell_width], topology)?,
        )?,
        Vec::new(),
    )
}

pub(super) fn generate(
    source: &CanonicalGeometryV1,
    source_correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
    quality_gate: MeshQualityGate,
) -> Result<GeneratedGmshMesh, Diagnostic> {
    let executable = gmsh_executable()?;
    let scratch = ScratchDirectory::create()?;
    require_version(&executable, scratch.path())?;

    let geometry_path = scratch.path().join("mesh.geo");
    let mesh_path = scratch.path().join("mesh.msh");
    let generated_geometry = geometry_script(source, source_correspondence)?;
    fs::write(&geometry_path, generated_geometry.script).map_err(|error| {
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
    let imported = importer.import_ascii_bytes_with_entities(&bytes)?;
    let edge_facets = generated_edge_facets(
        imported.mesh(),
        imported.element_blocks(),
        &generated_geometry.source_edge_by_tag,
    )?;
    let face_cells = generated_face_cells(
        imported.mesh(),
        imported.element_blocks(),
        &generated_geometry.source_face_by_tag,
    )?;
    let mesh = SimplicialMeshEnvelopeV1::from_mesh(imported.mesh())?;
    let correspondence =
        GeometryMeshCorrespondenceEnvelopeV1::from_planar_circular_hole_v2_mesh_assignments(
            source,
            &mesh,
            edge_facets.clone(),
        )?;
    Ok(GeneratedGmshMesh {
        mesh,
        correspondence,
        edge_facets,
        face_cells,
        element_blocks: imported.element_blocks().to_vec(),
        source_edge_by_tag: generated_geometry.source_edge_by_tag,
        source_face_by_tag: generated_geometry.source_face_by_tag,
    })
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

struct GeneratedGeometry {
    script: String,
    source_edge_by_tag: std::collections::BTreeMap<u32, usize>,
    source_face_by_tag: std::collections::BTreeMap<u32, usize>,
}

fn geometry_script(
    source: &CanonicalGeometryV1,
    source_correspondence: &GeometryMeshCorrespondenceEnvelopeV1,
) -> Result<GeneratedGeometry, Diagnostic> {
    let bounds = source
        .circular_hole_bounds()
        .ok_or_else(|| invalid_import("GmshMesher requires planar circular-hole Geometry v2"))?;
    let center = source
        .circular_hole_center()
        .ok_or_else(|| invalid_import("GmshMesher requires planar circular-hole Geometry v2"))?;
    let radius = source
        .circular_hole_radius_m()
        .ok_or_else(|| invalid_import("GmshMesher requires planar circular-hole Geometry v2"))?;
    CanonicalGeometryV1::decode_planar_circular_hole_v2_canonical(
        source.canonical_bytes(),
        eqiora::geometry::CanonicalGeometryLimits::default(),
    )
    .map_err(|_| invalid_import("GmshMesher requires planar circular-hole Geometry v2"))?;
    let mut script = String::from(
        "SetFactory(\"OpenCASCADE\");\n\
         General.NumThreads = 1;\n\
         Mesh.Algorithm = 6;\n\
         Mesh.ElementOrder = 1;\n\
         Mesh.Binary = 0;\n\
         Mesh.MshFileVersion = 4.1;\n\
         Mesh.RandomFactor = 0;\n\
         Mesh.SaveAll = 1;\n",
    );
    let [[x_min, x_max], [y_min, y_max]] = *bounds;
    for (tag, [x, y]) in [
        [x_min, y_min],
        [x_max, y_min],
        [x_max, y_max],
        [x_min, y_max],
    ]
    .into_iter()
    .enumerate()
    {
        writeln!(script, "Point({}) = {{{x:?}, {y:?}, 0}};", tag + 1)
            .expect("writing to String cannot fail");
    }
    writeln!(
        script,
        "Circle(1) = {{{:?}, {:?}, 0, {radius:?}}};",
        center[0], center[1]
    )
    .expect("writing to String cannot fail");
    for (tag, [start, end]) in [[1, 2], [2, 3], [3, 4], [4, 1]].into_iter().enumerate() {
        writeln!(script, "Line({}) = {{{start}, {end}}};", tag + 5)
            .expect("writing to String cannot fail");
    }
    writeln!(
        script,
        "Curve Loop(1) = {{-1}};\nCurve Loop(2) = {{5, 6, 7, 8}};\nPlane Surface(1) = {{2, 1}};"
    )
    .expect("writing to String cannot fail");

    // The exact source defines geometry.  The source-owned reference is used
    // only as the deterministic numerical sizing policy for Gmsh.
    let facet_counts = (0..5)
        .map(|source_edge| {
            source_correspondence
                .planar_circular_hole_v2_source_edge_entities(source, source_edge)
                .map(|facets| facets.len())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if facet_counts.contains(&0) {
        return Err(invalid_import(
            "Gmsh source sizing policy has an empty edge",
        ));
    }
    writeln!(script, "Transfinite Curve {{1}} = {};", facet_counts[4] + 1)
        .expect("writing to String cannot fail");
    // Gmsh counts nodes, rather than segments, on open transfinite curves.
    for (tag, source_edge) in [(5, 2), (6, 1), (7, 3), (8, 0)] {
        writeln!(
            script,
            "Transfinite Curve {{{tag}}} = {};",
            facet_counts[source_edge] + 1
        )
        .expect("writing to String cannot fail");
    }

    // Gmsh entity tags are construction-owned and mapped directly to the
    // five closed source-edge identities; no tag name or coordinate is used.
    let source_edge_by_tag =
        std::collections::BTreeMap::from([(1, 4), (5, 2), (6, 1), (7, 3), (8, 0)]);
    Ok(GeneratedGeometry {
        script,
        source_edge_by_tag,
        source_face_by_tag: std::collections::BTreeMap::from([(1, 0)]),
    })
}

fn generated_edge_facets(
    mesh: &eqiora::meshing::SimplicialMesh,
    element_blocks: &[eqiora::io::gmsh::GmshElementBlock],
    source_edge_by_tag: &std::collections::BTreeMap<u32, usize>,
) -> Result<[Vec<usize>; 5], Diagnostic> {
    let facet_count = mesh
        .entity_count(1)
        .ok_or_else(|| invalid_import("generated Gmsh Mesh has no facet stratum"))?;
    let mut facet_by_vertices = std::collections::BTreeMap::new();
    for facet_index in 0..facet_count {
        let mut vertices = mesh
            .entity_vertices(MeshEntity::new(1, facet_index))
            .ok_or_else(|| invalid_import("generated Gmsh facet has no vertex closure"))?
            .into_iter()
            .map(MeshEntity::index)
            .collect::<Vec<_>>();
        vertices.sort_unstable();
        facet_by_vertices.insert(vertices, facet_index);
    }
    let mut assigned: [Vec<usize>; 5] = std::array::from_fn(|_| Vec::new());
    for block in element_blocks.iter().filter(|block| block.dimension() == 1) {
        let source_edge = *source_edge_by_tag.get(&block.entity_tag()).ok_or_else(|| {
            invalid_import("generated Gmsh emitted an unknown boundary entity tag")
        })?;
        for element in block.elements() {
            let mut key = element.clone();
            key.sort_unstable();
            let facet = *facet_by_vertices.get(&key).ok_or_else(|| {
                invalid_import("generated Gmsh boundary element is absent from Mesh topology")
            })?;
            assigned[source_edge].push(facet);
        }
    }
    for facets in &mut assigned {
        facets.sort_unstable();
    }
    Ok(assigned)
}

fn generated_face_cells(
    mesh: &eqiora::meshing::SimplicialMesh,
    element_blocks: &[eqiora::io::gmsh::GmshElementBlock],
    source_face_by_tag: &std::collections::BTreeMap<u32, usize>,
) -> Result<Vec<usize>, Diagnostic> {
    if source_face_by_tag.values().copied().collect::<Vec<_>>() != [0] {
        return Err(invalid_import(
            "generated Gmsh source-face entity mapping is not canonical",
        ));
    }
    let mut cell_by_vertices = std::collections::BTreeMap::new();
    for (cell_index, cell) in mesh.cells().iter().enumerate() {
        let mut vertices = cell.clone();
        vertices.sort_unstable();
        if cell_by_vertices.insert(vertices, cell_index).is_some() {
            return Err(invalid_import(
                "generated Gmsh Mesh has duplicate cell connectivity",
            ));
        }
    }
    let mut assigned = std::collections::BTreeSet::new();
    for block in element_blocks.iter().filter(|block| block.dimension() == 2) {
        let source_face = source_face_by_tag
            .get(&block.entity_tag())
            .ok_or_else(|| invalid_import("generated Gmsh emitted an unknown face entity tag"))?;
        if *source_face != 0 {
            return Err(invalid_import(
                "generated Gmsh face entity maps to an unknown source face",
            ));
        }
        for element in block.elements() {
            let mut key = element.clone();
            key.sort_unstable();
            let cell = *cell_by_vertices.get(&key).ok_or_else(|| {
                invalid_import("generated Gmsh face element is absent from Mesh topology")
            })?;
            if !assigned.insert(cell) {
                return Err(invalid_import(
                    "generated Gmsh face entity assigns one Mesh cell more than once",
                ));
            }
        }
    }
    let cells = assigned.into_iter().collect::<Vec<_>>();
    if cells != (0..mesh.cells().len()).collect::<Vec<_>>() {
        return Err(invalid_import(
            "generated Gmsh face entity omits a Mesh cell",
        ));
    }
    Ok(cells)
}

pub(super) fn revalidate_generated(
    source: &CanonicalGeometryV1,
    generated: &GeneratedGmshMesh,
) -> Result<(), Diagnostic> {
    let edge_facets = generated_edge_facets(
        generated.mesh.mesh(),
        &generated.element_blocks,
        &generated.source_edge_by_tag,
    )?;
    if edge_facets != generated.edge_facets {
        return Err(invalid_import(
            "retained Gmsh entity-block provenance differs from source assignments",
        ));
    }
    let face_cells = generated_face_cells(
        generated.mesh.mesh(),
        &generated.element_blocks,
        &generated.source_face_by_tag,
    )?;
    if face_cells != generated.face_cells {
        return Err(invalid_import(
            "retained Gmsh face provenance differs from source assignment",
        ));
    }
    generated
        .correspondence
        .validate_against_planar_circular_hole_v2_mesh_assignments(
            source,
            &generated.mesh,
            edge_facets,
        )
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
