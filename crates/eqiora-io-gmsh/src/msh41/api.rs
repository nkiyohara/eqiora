use std::collections::{BTreeMap, HashMap, HashSet};

use eqiora_core::Diagnostic;
use eqiora_meshing::{MeshEntity, MeshQualityGate, MeshTopology, SimplicialMesh};

use super::{Decoder, DecoderLimits, invalid_import};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssignmentMode {
    Discard,
    RequireCompleteAscii,
}

/// Complete bounded-decoder and simplex-acceptance policy for MSH 4.1.
///
/// Use [`Self::mesh`] for an unlabelled imported Mesh, or
/// [`Self::ascii_with_entity_assignments`] when a provider must retain exact
/// geometric entity tags as assignments to accepted Mesh entities. Resource
/// fields are caller-owned and may be raised together for trusted workloads.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Msh41Policy {
    dimension: usize,
    quality_gate: MeshQualityGate,
    assignment_mode: AssignmentMode,
    /// Maximum complete input size.
    pub max_bytes: usize,
    /// Maximum number of decoded geometric-entity records.
    pub max_entities: usize,
    /// Maximum total decoded physical and boundary references in `$Entities`.
    pub max_entity_references: usize,
    /// Maximum decoded node-block count.
    pub max_node_blocks: usize,
    /// Maximum decoded element-block count.
    pub max_element_blocks: usize,
    /// Maximum decoded node count.
    pub max_nodes: usize,
    /// Maximum decoded element count, including ignored lower-dimensional elements.
    pub max_elements: usize,
    /// Maximum lower-dimensional elements decoded but omitted from canonical cells.
    pub max_ignored_elements: usize,
    /// Maximum aggregate logical bytes for decoder and canonical mesh state.
    pub max_decoded_bytes: usize,
    /// Maximum aggregate decode, lookup, and topology-construction work units.
    pub max_decoded_work: usize,
}

impl Msh41Policy {
    /// Construct the default bounded policy for an unlabelled XY triangle or
    /// XYZ tetrahedron Mesh.
    ///
    /// # Errors
    /// Returns `EQ0808` for a dimension other than two or three.
    pub fn mesh(dimension: usize, quality_gate: MeshQualityGate) -> Result<Self, Diagnostic> {
        Self::new(dimension, quality_gate, AssignmentMode::Discard)
    }

    /// Construct the default bounded policy for an ASCII Mesh whose complete
    /// boundary and top-dimensional geometric entity assignments are required.
    ///
    /// # Errors
    /// Returns `EQ0808` for a dimension other than two or three.
    pub fn ascii_with_entity_assignments(
        dimension: usize,
        quality_gate: MeshQualityGate,
    ) -> Result<Self, Diagnostic> {
        Self::new(
            dimension,
            quality_gate,
            AssignmentMode::RequireCompleteAscii,
        )
    }

    fn new(
        dimension: usize,
        quality_gate: MeshQualityGate,
        assignment_mode: AssignmentMode,
    ) -> Result<Self, Diagnostic> {
        if !matches!(dimension, 2 | 3) {
            return Err(invalid_import(
                "Gmsh simplex import supports spatial dimension two or three",
            ));
        }
        let limits = DecoderLimits::default();
        Ok(Self {
            dimension,
            quality_gate,
            assignment_mode,
            max_bytes: limits.max_bytes,
            max_entities: limits.max_entities,
            max_entity_references: limits.max_entity_references,
            max_node_blocks: limits.max_node_blocks,
            max_element_blocks: limits.max_element_blocks,
            max_nodes: limits.max_nodes,
            max_elements: limits.max_elements,
            max_ignored_elements: limits.max_ignored_elements,
            max_decoded_bytes: limits.max_decoded_bytes,
            max_decoded_work: limits.max_decoded_work,
        })
    }

    fn limits(self) -> Result<DecoderLimits, Diagnostic> {
        DecoderLimits {
            max_bytes: self.max_bytes,
            max_entities: self.max_entities,
            max_entity_references: self.max_entity_references,
            max_node_blocks: self.max_node_blocks,
            max_element_blocks: self.max_element_blocks,
            max_nodes: self.max_nodes,
            max_elements: self.max_elements,
            max_ignored_elements: self.max_ignored_elements,
            max_decoded_bytes: self.max_decoded_bytes,
            max_decoded_work: self.max_decoded_work,
        }
        .validate()
    }
}

/// One MSH element block with its exact geometric-entity provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct DecodedElementBlock {
    pub(super) dimension: usize,
    pub(super) entity_tag: u32,
    pub(super) elements: Vec<Vec<usize>>,
}

impl DecodedElementBlock {
    /// Geometric entity dimension declared by the MSH block.
    #[must_use]
    pub const fn dimension(&self) -> usize {
        self.dimension
    }

    /// Positive geometric entity tag declared by the MSH block.
    #[must_use]
    pub const fn entity_tag(&self) -> u32 {
        self.entity_tag
    }

    /// Element vertex indices in importer-normalized Mesh vertex order.
    #[must_use]
    pub fn elements(&self) -> &[Vec<usize>] {
        &self.elements
    }
}

/// An admitted ASCII MSH mesh together with its element-block provenance.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct DecodedMsh {
    pub(super) mesh: SimplicialMesh,
    pub(super) element_blocks: Vec<DecodedElementBlock>,
}

impl DecodedMsh {
    /// Common accepted simplex Mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }

    /// Complete source element-block provenance in MSH order.
    #[must_use]
    pub fn element_blocks(&self) -> &[DecodedElementBlock] {
        &self.element_blocks
    }

    /// Consume the import and return its common Mesh.
    #[must_use]
    pub fn into_mesh(self) -> SimplicialMesh {
        self.mesh
    }
}

/// Decode one bounded MSH 4.1 image into the common simplex Mesh contract.
///
/// The assignment sink is not called for [`Msh41Policy::mesh`]. The
/// `ascii_with_entity_assignments` policy instead validates complete boundary
/// and top-dimensional coverage, then calls the sink once per canonical
/// `(Mesh dimension, source entity tag, Mesh indices)` group. No partial
/// assignments escape a rejected import. Filesystem and provider provenance
/// remain caller-owned.
///
/// # Errors
/// Returns `EQ0808` when bytes, declarations, assignments, or resource use
/// exceed the selected policy, and `EQ0803` when the common Mesh contract
/// rejects topology, geometry, orientation, or quality.
pub fn import_msh41(
    bytes: &[u8],
    policy: Msh41Policy,
    mut receive_assignments: impl FnMut(usize, u32, &[usize]),
) -> Result<SimplicialMesh, Diagnostic> {
    let importer = Decoder::new(policy.dimension, policy.quality_gate, policy.limits()?)?;
    match policy.assignment_mode {
        AssignmentMode::Discard => importer.import_bytes(bytes),
        AssignmentMode::RequireCompleteAscii => {
            let imported = importer.import_ascii_bytes_with_entities(bytes)?;
            let assignments = derive_entity_assignments(&imported)?;
            for ((dimension, tag), indices) in assignments {
                receive_assignments(dimension, tag, &indices);
            }
            Ok(imported.into_mesh())
        }
    }
}

fn derive_entity_assignments(
    imported: &DecodedMsh,
) -> Result<BTreeMap<(usize, u32), Vec<usize>>, Diagnostic> {
    let mesh = imported.mesh();
    let dimension = mesh.topological_dimension();
    let facet_dimension = dimension
        .checked_sub(1)
        .ok_or_else(|| invalid_import("Gmsh simplex Mesh has no boundary stratum"))?;
    let facet_count = mesh
        .entity_count(facet_dimension)
        .ok_or_else(|| invalid_import("Gmsh simplex Mesh omitted its facet stratum"))?;
    let mut facet_by_vertices = HashMap::new();
    let mut boundary_facets = HashSet::new();
    for facet_index in 0..facet_count {
        let facet = MeshEntity::new(facet_dimension, facet_index);
        let mut vertices = mesh
            .entity_vertices(facet)
            .ok_or_else(|| invalid_import("Gmsh Mesh facet omitted its vertex closure"))?
            .into_iter()
            .map(MeshEntity::index)
            .collect::<Vec<_>>();
        vertices.sort_unstable();
        if facet_by_vertices.insert(vertices, facet).is_some() {
            return Err(invalid_import(
                "Gmsh Mesh has duplicate canonical facet connectivity",
            ));
        }
        let parents = mesh
            .incidence(facet, dimension)
            .ok_or_else(|| invalid_import("Gmsh Mesh facet omitted parent incidence"))?;
        if parents.len() == 1 {
            boundary_facets.insert(facet);
        }
    }

    let mut assignments: BTreeMap<(usize, u32), Vec<usize>> = BTreeMap::new();
    let mut assigned_facets = HashSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == facet_dimension)
    {
        let facets = assignments
            .entry((facet_dimension, block.entity_tag()))
            .or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let facet = *facet_by_vertices.get(&vertices).ok_or_else(|| {
                invalid_import("Gmsh boundary element is absent from Mesh topology")
            })?;
            if !boundary_facets.contains(&facet) || !assigned_facets.insert(facet) {
                return Err(invalid_import(
                    "Gmsh boundary assignment is interior or duplicated",
                ));
            }
            facets.push(facet.index());
        }
    }
    if assigned_facets != boundary_facets {
        return Err(invalid_import(
            "Gmsh entity blocks do not assign every Mesh boundary facet",
        ));
    }

    let mut cell_by_vertices = HashMap::new();
    for (cell_index, cell) in mesh.cells().iter().enumerate() {
        let mut vertices = cell.clone();
        vertices.sort_unstable();
        let entity = MeshEntity::new(dimension, cell_index);
        if cell_by_vertices.insert(vertices, entity).is_some() {
            return Err(invalid_import("Gmsh Mesh has duplicate canonical cells"));
        }
    }
    let mut assigned_cells = HashSet::new();
    for block in imported
        .element_blocks()
        .iter()
        .filter(|block| block.dimension() == dimension)
    {
        let cells = assignments
            .entry((dimension, block.entity_tag()))
            .or_default();
        for element in block.elements() {
            let mut vertices = element.clone();
            vertices.sort_unstable();
            let cell = *cell_by_vertices
                .get(&vertices)
                .ok_or_else(|| invalid_import("Gmsh top element is absent from Mesh topology"))?;
            if !assigned_cells.insert(cell) {
                return Err(invalid_import("Gmsh top cell assignment is duplicated"));
            }
            cells.push(cell.index());
        }
    }
    for indices in assignments.values_mut() {
        indices.sort_unstable();
    }
    let all_cells = (0..mesh.cells().len())
        .map(|index| MeshEntity::new(dimension, index))
        .collect();
    if assigned_cells != all_cells {
        return Err(invalid_import(
            "Gmsh entity blocks do not assign every Mesh top cell",
        ));
    }
    Ok(assignments)
}
