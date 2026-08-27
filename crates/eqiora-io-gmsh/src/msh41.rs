use std::collections::{HashMap, HashSet};
use std::hash::Hash;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};

mod binary;
mod budget;

use budget::DecodedBudget;

const MSH_VERSION: &str = "4.1";

/// Decoded-work and input-size policy applied by the owned bounded decoder.
///
/// Every declared loop is charged to one of these semantic count budgets.
/// Binary counts must also fit the minimum records encodable in the remaining
/// input. Aggregate decoded-byte and work budgets conservatively cover
/// structural indexes, decoded coordinates and connectivity, lookup state,
/// canonical output, and simplex topology closure; declaration-sized importer
/// storage also uses fallible reservation. The byte budget is a deterministic
/// logical account, not a promise of exact allocator RSS.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GmshImportLimits {
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

impl Default for GmshImportLimits {
    fn default() -> Self {
        Self {
            max_bytes: 16 * 1024 * 1024,
            max_entities: 1_000_000,
            max_entity_references: 8_000_000,
            max_node_blocks: 100_000,
            max_element_blocks: 100_000,
            max_nodes: 1_000_000,
            max_elements: 2_000_000,
            max_ignored_elements: 16_384,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_decoded_work: 32_000_000,
        }
    }
}

impl GmshImportLimits {
    fn validate(self) -> Result<Self, Diagnostic> {
        if [
            self.max_bytes,
            self.max_entities,
            self.max_entity_references,
            self.max_node_blocks,
            self.max_element_blocks,
            self.max_nodes,
            self.max_elements,
            self.max_ignored_elements,
            self.max_decoded_bytes,
            self.max_decoded_work,
        ]
        .contains(&0)
        {
            return Err(invalid_import(
                "Gmsh import resource limits must all be positive",
            ));
        }
        Ok(self)
    }
}

/// Strict importer for full-dimensional affine simplex meshes in MSH 4.1.
///
/// This type accepts bytes rather than paths so filesystem policy and source
/// provenance stay outside accepted mesh identity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GmshSimplexImporter {
    dimension: usize,
    quality_gate: MeshQualityGate,
    limits: GmshImportLimits,
}

/// One MSH element block with its exact geometric-entity provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmshElementBlock {
    dimension: usize,
    entity_tag: u32,
    elements: Vec<Vec<usize>>,
}

impl GmshElementBlock {
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
pub struct GmshSimplicialImport {
    mesh: SimplicialMesh,
    element_blocks: Vec<GmshElementBlock>,
}

impl GmshSimplicialImport {
    /// Common accepted simplex Mesh.
    #[must_use]
    pub const fn mesh(&self) -> &SimplicialMesh {
        &self.mesh
    }

    /// Complete source element-block provenance in MSH order.
    #[must_use]
    pub fn element_blocks(&self) -> &[GmshElementBlock] {
        &self.element_blocks
    }

    /// Consume the import and return its common Mesh.
    #[must_use]
    pub fn into_mesh(self) -> SimplicialMesh {
        self.mesh
    }
}

impl GmshSimplexImporter {
    /// Construct an importer for XY triangles (`dimension = 2`) or XYZ
    /// tetrahedra (`dimension = 3`).
    ///
    /// # Errors
    /// Returns `EQ0808` for an unsupported dimension or zero resource limit.
    pub fn new(
        dimension: usize,
        quality_gate: MeshQualityGate,
        limits: GmshImportLimits,
    ) -> Result<Self, Diagnostic> {
        if !matches!(dimension, 2 | 3) {
            return Err(invalid_import(
                "Gmsh simplex import supports spatial dimension two or three",
            ));
        }
        Ok(Self {
            dimension,
            quality_gate,
            limits: limits.validate()?,
        })
    }

    /// Requested full-dimensional mesh dimension.
    #[must_use]
    pub const fn dimension(self) -> usize {
        self.dimension
    }

    /// Resource policy applied throughout bounded decoding.
    #[must_use]
    pub const fn limits(self) -> GmshImportLimits {
        self.limits
    }

    /// Parse, validate, and reconstruct one admitted MSH 4.1 mesh.
    ///
    /// # Errors
    /// Returns `EQ0808` when the bytes exceed the admitted syntax, semantics,
    /// or resource boundary. Accepted syntax may still return `EQ0803` when
    /// the shared mesh contract rejects topology, geometry, or quality.
    pub fn import_bytes(self, bytes: &[u8]) -> Result<SimplicialMesh, Diagnostic> {
        self.decode(bytes).and_then(|decoded| {
            SimplicialMesh::new(
                self.dimension,
                decoded.vertices,
                decoded.cells,
                self.quality_gate,
            )
        })
    }

    /// Parse an ASCII MSH 4.1 image while retaining its element-block entity
    /// tags and normalized connectivity.
    ///
    /// Generated-provider adapters use this closed provenance path to map CAD
    /// entities to Mesh entities without coordinate classification. Caller
    /// imports that do not authenticate entity meaning should use
    /// [`Self::import_bytes`] and remain unlabelled.
    pub fn import_ascii_bytes_with_entities(
        self,
        bytes: &[u8],
    ) -> Result<GmshSimplicialImport, Diagnostic> {
        if declared_encoding(bytes)? != InputEncoding::Ascii {
            return Err(invalid_import(
                "entity-provenance import requires ASCII MSH 4.1",
            ));
        }
        let decoded = self.decode(bytes)?;
        let mesh = SimplicialMesh::new(
            self.dimension,
            decoded.vertices,
            decoded.cells,
            self.quality_gate,
        )?;
        Ok(GmshSimplicialImport {
            mesh,
            element_blocks: decoded.element_blocks,
        })
    }

    fn decode(self, bytes: &[u8]) -> Result<DecodedMesh, Diagnostic> {
        if bytes.len() > self.limits.max_bytes {
            return Err(invalid_import(
                "Gmsh input exceeds the configured byte limit",
            ));
        }
        let encoding = declared_encoding(bytes)?;
        let mut decoded_budget = DecodedBudget::new(self.limits);
        let decoded = match encoding {
            InputEncoding::Ascii => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| invalid_import("ASCII MSH 4.1 input must be valid UTF-8"))?;
                DecodedMesh::parse_ascii(text, self.dimension, self.limits, &mut decoded_budget)?
            }
            InputEncoding::Binary { size_t_size } => binary::parse(
                bytes,
                self.dimension,
                self.limits,
                size_t_size,
                &mut decoded_budget,
            )?,
        };
        Ok(decoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputEncoding {
    Ascii,
    Binary { size_t_size: usize },
}

fn declared_encoding(bytes: &[u8]) -> Result<InputEncoding, Diagnostic> {
    let mut lines = bytes.splitn(3, |&byte| byte == b'\n');
    let section = strip_carriage_return(
        lines
            .next()
            .ok_or_else(|| invalid_import("MSH input has no format section"))?,
    );
    if section != b"$MeshFormat" {
        return Err(invalid_import("MSH input must begin with $MeshFormat"));
    }
    let header = strip_carriage_return(
        lines
            .next()
            .ok_or_else(|| invalid_import("MSH input has no format header"))?,
    );
    let header = std::str::from_utf8(header)
        .map_err(|_| invalid_import("MSH format header must be ASCII"))?;
    let [version, file_type, size_t_size] = exact_fields::<3>(Some(header), "$MeshFormat header")?;
    if version != MSH_VERSION {
        return Err(invalid_import("Gmsh import requires MSH version 4.1"));
    }
    let file_type = parse_usize(file_type, "$MeshFormat file type")?;
    let size_t_size = parse_usize(size_t_size, "$MeshFormat data size")?;
    match (file_type, size_t_size) {
        (0, 8) => Ok(InputEncoding::Ascii),
        (1, 4 | 8) => Ok(InputEncoding::Binary { size_t_size }),
        (0, _) => Err(invalid_import(
            "ASCII MSH 4.1 import requires the data-size declaration 8",
        )),
        (1, _) => Err(invalid_import(
            "binary MSH 4.1 import requires a four- or eight-byte size_t declaration",
        )),
        _ => Err(invalid_import(
            "MSH 4.1 file type must be zero for ASCII or one for binary",
        )),
    }
}

fn strip_carriage_return(line: &[u8]) -> &[u8] {
    line.strip_suffix(b"\r").unwrap_or(line)
}

#[derive(Debug)]
struct DecodedMesh {
    vertices: Vec<Vec<f64>>,
    cells: Vec<Vec<usize>>,
    element_blocks: Vec<GmshElementBlock>,
}

#[derive(Debug)]
struct DecodedNodes {
    vertices: Vec<Vec<f64>>,
    vertex_by_tag: HashMap<u64, usize>,
}

impl DecodedMesh {
    fn parse_ascii(
        text: &str,
        dimension: usize,
        limits: GmshImportLimits,
        budget: &mut DecodedBudget,
    ) -> Result<Self, Diagnostic> {
        let sections = Sections::parse(text, budget)?;
        sections.validate_admitted_order()?;
        parse_mesh_format(sections.required("MeshFormat")?)?;
        if let Some(entities) = sections.optional("Entities")? {
            parse_entities(entities, dimension, limits, budget)?;
        }
        let nodes = parse_nodes(sections.required("Nodes")?, dimension, limits, budget)?;
        let (cells, element_blocks) = parse_elements(
            sections.required("Elements")?,
            dimension,
            limits,
            &nodes.vertex_by_tag,
            budget,
        )?;
        Ok(Self {
            vertices: nodes.vertices,
            cells,
            element_blocks,
        })
    }
}

#[derive(Debug)]
struct Section<'a> {
    name: &'a str,
    lines: Vec<&'a str>,
}

#[derive(Debug)]
struct Sections<'a>(Vec<Section<'a>>);

impl<'a> Sections<'a> {
    fn parse(text: &'a str, budget: &mut DecodedBudget) -> Result<Self, Diagnostic> {
        let line_count = text.lines().count();
        budget.charge_ascii_lines(line_count)?;
        let mut lines = fallible_vec(line_count, "ASCII line-index storage")?;
        lines.extend(text.lines().map(str::trim));
        let mut sections = fallible_vec(4, "MSH section-index storage")?;
        let mut index = 0;
        while index < lines.len() {
            if lines[index].is_empty() {
                index = checked_next_index(index, "MSH line cursor")?;
                continue;
            }
            let start = lines[index];
            let Some(name) = start.strip_prefix('$') else {
                return Err(invalid_import("text outside a delimited MSH section"));
            };
            if name.is_empty() || name.starts_with("End") {
                return Err(invalid_import("invalid MSH section delimiter"));
            }
            index = checked_next_index(index, "MSH section cursor")?;
            let mut body = Vec::new();
            while index < lines.len() && lines[index].strip_prefix("$End") != Some(name) {
                if lines[index].starts_with('$') {
                    return Err(invalid_import("nested or mismatched MSH section delimiter"));
                }
                if !lines[index].is_empty() {
                    fallible_push(&mut body, lines[index], "MSH section-body storage")?;
                }
                index = checked_next_index(index, "MSH section-body cursor")?;
            }
            if index == lines.len() {
                return Err(invalid_import("unterminated MSH section"));
            }
            fallible_push(
                &mut sections,
                Section { name, lines: body },
                "MSH section-index storage",
            )?;
            index = checked_next_index(index, "MSH section-end cursor")?;
        }
        Ok(Self(sections))
    }

    fn required(&self, name: &str) -> Result<&[&'a str], Diagnostic> {
        self.optional(name)?
            .ok_or_else(|| invalid_import(format!("MSH input requires one ${name} section")))
    }

    fn optional(&self, name: &str) -> Result<Option<&[&'a str]>, Diagnostic> {
        let mut matching = self.0.iter().filter(|section| section.name == name);
        let first = matching.next();
        if matching.next().is_some() {
            return Err(invalid_import(format!(
                "MSH input contains duplicate ${name} sections"
            )));
        }
        Ok(first.map(|section| section.lines.as_slice()))
    }

    fn validate_admitted_order(&self) -> Result<(), Diagnostic> {
        let admitted = match self.0.as_slice() {
            [format, nodes, elements] => {
                format.name == "MeshFormat" && nodes.name == "Nodes" && elements.name == "Elements"
            }
            [format, entities, nodes, elements] => {
                format.name == "MeshFormat"
                    && entities.name == "Entities"
                    && nodes.name == "Nodes"
                    && elements.name == "Elements"
            }
            _ => false,
        };
        if admitted {
            Ok(())
        } else {
            Err(invalid_import(
                "MSH input must contain only one ordered $MeshFormat, optional $Entities, $Nodes, and $Elements section",
            ))
        }
    }
}

fn parse_mesh_format(lines: &[&str]) -> Result<(), Diagnostic> {
    if lines.len() != 1 {
        return Err(invalid_import(
            "$MeshFormat must contain exactly one header line",
        ));
    }
    let fields = exact_fields::<3>(Some(lines[0]), "$MeshFormat header")?;
    if fields != [MSH_VERSION, "0", "8"] {
        return Err(invalid_import(
            "Gmsh import requires the ASCII MSH 4.1 header `4.1 0 8`",
        ));
    }
    Ok(())
}

fn parse_entities(
    lines: &[&str],
    requested_dimension: usize,
    limits: GmshImportLimits,
    budget: &mut DecodedBudget,
) -> Result<(), Diagnostic> {
    let header = exact_fields::<4>(lines.first().copied(), "$Entities header")?;
    let counts = [
        parse_usize(header[0], "$Entities point count")?,
        parse_usize(header[1], "$Entities curve count")?,
        parse_usize(header[2], "$Entities surface count")?,
        parse_usize(header[3], "$Entities volume count")?,
    ];
    if counts
        .iter()
        .enumerate()
        .any(|(dimension, &count)| dimension > requested_dimension && count != 0)
    {
        return Err(invalid_import(
            "$Entities contains geometry above the requested mesh dimension",
        ));
    }
    let total = checked_sum(&counts, "$Entities count")?;
    let record_lines = total
        .checked_add(1)
        .ok_or_else(|| invalid_import("$Entities record count overflows usize"))?;
    if total > limits.max_entities || lines.len() != record_lines {
        return Err(invalid_import(
            "$Entities exceeds its limit or disagrees with its declared count",
        ));
    }
    budget.charge_entities(&counts)?;
    let mut references = 0usize;
    let mut entity_tags = [
        fallible_set(counts[0], "$Entities point-tag storage")?,
        fallible_set(counts[1], "$Entities curve-tag storage")?,
        fallible_set(counts[2], "$Entities surface-tag storage")?,
        fallible_set(counts[3], "$Entities volume-tag storage")?,
    ];
    let mut line_index = 1_usize;
    for (dimension, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Entities record")?;
            let mut tokens = TokenCursor::new(line);
            let tag = parse_i32(tokens.next("$Entities tag")?, "$Entities tag")?;
            if tag <= 0 || !entity_tags[dimension].insert(tag) {
                return Err(invalid_import(
                    "entity tags must be positive and unique per dimension",
                ));
            }
            let coordinate_count = if dimension == 0 { 3 } else { 6 };
            for _ in 0..coordinate_count {
                parse_finite(tokens.next("$Entities coordinate")?, "$Entities coordinate")?;
            }
            let physical_count = parse_usize(
                tokens.next("$Entities physical tag count")?,
                "$Entities physical tag count",
            )?;
            if physical_count != 0 {
                return Err(invalid_import(
                    "physical-group membership is outside the admitted import boundary",
                ));
            }
            if dimension > 0 {
                let boundary_count = parse_usize(
                    tokens.next("$Entities boundary count")?,
                    "$Entities boundary count",
                )?;
                references = references
                    .checked_add(boundary_count)
                    .ok_or_else(|| invalid_import("$Entities reference count overflows"))?;
                if references > limits.max_entity_references {
                    return Err(invalid_import(
                        "$Entities boundary references exceed the configured limit",
                    ));
                }
                budget.charge_entity_references(boundary_count)?;
                for _ in 0..boundary_count {
                    if parse_i32(
                        tokens.next("$Entities boundary tag")?,
                        "$Entities boundary tag",
                    )? == 0
                    {
                        return Err(invalid_import("entity boundary tags must be nonzero"));
                    }
                }
            }
            tokens.finish("$Entities record")?;
        }
    }
    Ok(())
}

fn parse_nodes(
    lines: &[&str],
    dimension: usize,
    limits: GmshImportLimits,
    budget: &mut DecodedBudget,
) -> Result<DecodedNodes, Diagnostic> {
    let header = exact_fields::<4>(lines.first().copied(), "$Nodes header")?;
    let block_count = parse_usize(header[0], "$Nodes block count")?;
    let total_nodes = parse_usize(header[1], "$Nodes total count")?;
    let declared_min = parse_u64(header[2], "$Nodes minimum tag")?;
    let declared_max = parse_u64(header[3], "$Nodes maximum tag")?;
    if block_count == 0
        || total_nodes == 0
        || block_count > limits.max_node_blocks
        || total_nodes > limits.max_nodes
        || block_count > lines.len()
        || total_nodes > lines.len()
    {
        return Err(invalid_import(
            "$Nodes count is zero or exceeds its resource limit",
        ));
    }
    budget.charge_nodes(block_count, total_nodes, dimension)?;

    let mut line_index = 1;
    let mut vertices = fallible_vec(total_nodes, "decoded vertex storage")?;
    let mut vertex_by_tag = fallible_map(total_nodes, "$Nodes tag-to-vertex storage")?;
    let mut observed_min = u64::MAX;
    let mut observed_max = 0_u64;
    let mut checked_total = 0usize;
    for _ in 0..block_count {
        let block_header = exact_fields::<4>(
            Some(take_declared_line(
                lines,
                &mut line_index,
                "$Nodes block header",
            )?),
            "$Nodes block header",
        )?;
        let entity_dim = parse_i32(block_header[0], "$Nodes entity dimension")?;
        let entity_tag = parse_i32(block_header[1], "$Nodes entity tag")?;
        let parametric = parse_usize(block_header[2], "$Nodes parametric flag")?;
        let count = parse_usize(block_header[3], "$Nodes block count")?;
        if entity_dim < 0 || entity_dim as usize > dimension || entity_tag <= 0 || parametric != 0 {
            return Err(invalid_import(
                "node blocks require an admitted entity dimension, positive tag, and no parametric coordinates",
            ));
        }
        checked_total = checked_total
            .checked_add(count)
            .ok_or_else(|| invalid_import("$Nodes block counts overflow usize"))?;
        if checked_total > total_nodes {
            return Err(invalid_import(
                "$Nodes block counts exceed the declared bounded total",
            ));
        }
        let block_start = vertices.len();
        for offset in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Nodes node tag")?;
            let [value] = exact_fields::<1>(Some(line), "$Nodes node tag")?;
            let tag = parse_u64(value, "$Nodes node tag")?;
            let vertex_index = block_start
                .checked_add(offset)
                .ok_or_else(|| invalid_import("$Nodes vertex index overflows usize"))?;
            if tag == 0 || vertex_by_tag.insert(tag, vertex_index).is_some() {
                return Err(invalid_import("node tags must be positive and unique"));
            }
            observed_min = observed_min.min(tag);
            observed_max = observed_max.max(tag);
        }
        for _ in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Nodes coordinate")?;
            let coordinate = exact_fields::<3>(Some(line), "$Nodes coordinate")?;
            let x = parse_finite(coordinate[0], "$Nodes x coordinate")?;
            let y = parse_finite(coordinate[1], "$Nodes y coordinate")?;
            let z = parse_finite(coordinate[2], "$Nodes z coordinate")?;
            vertices.push(decoded_coordinate(dimension, x, y, z)?);
        }
    }
    if line_index != lines.len()
        || checked_total != total_nodes
        || vertices.len() != total_nodes
        || vertex_by_tag.len() != total_nodes
        || observed_min != declared_min
        || observed_max != declared_max
    {
        return Err(invalid_import(
            "$Nodes records disagree with declared totals or tag bounds",
        ));
    }
    Ok(DecodedNodes {
        vertices,
        vertex_by_tag,
    })
}

fn parse_elements(
    lines: &[&str],
    dimension: usize,
    limits: GmshImportLimits,
    vertex_by_tag: &HashMap<u64, usize>,
    budget: &mut DecodedBudget,
) -> Result<(Vec<Vec<usize>>, Vec<GmshElementBlock>), Diagnostic> {
    let header = exact_fields::<4>(lines.first().copied(), "$Elements header")?;
    let block_count = parse_usize(header[0], "$Elements block count")?;
    let total_elements = parse_usize(header[1], "$Elements total count")?;
    let declared_min = parse_u64(header[2], "$Elements minimum tag")?;
    let declared_max = parse_u64(header[3], "$Elements maximum tag")?;
    if block_count == 0
        || total_elements == 0
        || block_count > limits.max_element_blocks
        || total_elements > limits.max_elements
        || block_count > lines.len()
        || total_elements > lines.len()
    {
        return Err(invalid_import(
            "$Elements count is zero or exceeds its resource limit",
        ));
    }
    budget.charge_element_blocks(block_count)?;

    let mut line_index = 1;
    let mut cells = Vec::new();
    let mut element_blocks = fallible_vec(block_count, "$Elements provenance-block storage")?;
    let mut all_tags = fallible_set(total_elements, "$Elements unique-tag storage")?;
    let mut observed_min = u64::MAX;
    let mut observed_max = 0_u64;
    let mut checked_total = 0usize;
    let mut top_dimensional_elements = 0_usize;
    for _ in 0..block_count {
        let block_header = exact_fields::<4>(
            Some(take_declared_line(
                lines,
                &mut line_index,
                "$Elements block header",
            )?),
            "$Elements block header",
        )?;
        let entity_dim = parse_i32(block_header[0], "$Elements entity dimension")?;
        let entity_tag = parse_i32(block_header[1], "$Elements entity tag")?;
        let element_type_usize = parse_usize(block_header[2], "$Elements element type")?;
        let element_type = u32::try_from(element_type_usize)
            .map_err(|_| invalid_import("MSH element type exceeds u32"))?;
        let count = parse_usize(block_header[3], "$Elements block count")?;
        if entity_dim < 0 || entity_dim as usize > dimension || entity_tag <= 0 {
            return Err(invalid_import(
                "element blocks require an admitted entity dimension and positive tag",
            ));
        }
        let entity_dimension = entity_dim as usize;
        let entity_tag =
            u32::try_from(entity_tag).map_err(|_| invalid_import("MSH entity tag exceeds u32"))?;
        if element_type != linear_simplex_type(entity_dimension) {
            return Err(invalid_import(
                "element blocks must use the linear simplex type of their entity dimension",
            ));
        }
        checked_total = checked_total
            .checked_add(count)
            .ok_or_else(|| invalid_import("$Elements block counts overflow usize"))?;
        if checked_total > total_elements {
            return Err(invalid_import(
                "$Elements block counts exceed the declared bounded total",
            ));
        }
        budget.charge_elements(count, entity_dimension, dimension)?;
        if entity_dimension == dimension {
            top_dimensional_elements = top_dimensional_elements
                .checked_add(count)
                .ok_or_else(|| invalid_import("top-dimensional element count overflows usize"))?;
            fallible_reserve(&mut cells, count, "decoded top-dimensional cell storage")?;
        }
        let mut block_elements = fallible_vec(count, "$Elements provenance storage")?;
        for _ in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Elements record")?;
            let mut tokens = TokenCursor::new(line);
            let tag = parse_u64(
                tokens.next("$Elements element tag")?,
                "$Elements element tag",
            )?;
            if tag == 0 || !all_tags.insert(tag) {
                return Err(invalid_import("element tags must be positive and unique"));
            }
            observed_min = observed_min.min(tag);
            observed_max = observed_max.max(tag);
            let mut element =
                fallible_vec(entity_dimension + 1, "decoded simplex-connectivity storage")?;
            for _ in 0..=entity_dimension {
                let node_tag = parse_u64(tokens.next("$Elements node tag")?, "$Elements node tag")?;
                if node_tag == 0 {
                    return Err(invalid_import("element node tags must be positive"));
                }
                let vertex = vertex_by_tag
                    .get(&node_tag)
                    .copied()
                    .ok_or_else(|| invalid_import("MSH element references an unknown node tag"))?;
                element.push(vertex);
            }
            tokens.finish("$Elements record")?;
            if entity_dimension == dimension {
                fallible_push(
                    &mut cells,
                    element.clone(),
                    "decoded top-dimensional cell storage",
                )?;
            }
            fallible_push(&mut block_elements, element, "$Elements provenance storage")?;
        }
        fallible_push(
            &mut element_blocks,
            GmshElementBlock {
                dimension: entity_dimension,
                entity_tag,
                elements: block_elements,
            },
            "$Elements provenance-block storage",
        )?;
    }
    if line_index != lines.len()
        || checked_total != total_elements
        || all_tags.len() != total_elements
        || observed_min != declared_min
        || observed_max != declared_max
        || cells.len() != top_dimensional_elements
    {
        return Err(invalid_import(
            "$Elements records disagree with declared totals or tag bounds",
        ));
    }
    if cells.is_empty() {
        return Err(invalid_import(
            "MSH input contains no admitted top-dimensional simplex cells",
        ));
    }
    Ok((cells, element_blocks))
}

const fn linear_simplex_type(dimension: usize) -> u32 {
    match dimension {
        0 => 15,
        1 => 1,
        2 => 2,
        3 => 4,
        _ => 0,
    }
}

fn exact_fields<'a, const COUNT: usize>(
    line: Option<&'a str>,
    context: &str,
) -> Result<[&'a str; COUNT], Diagnostic> {
    let line = line.ok_or_else(|| invalid_import(format!("missing {context}")))?;
    let mut tokens = TokenCursor::new(line);
    let mut values = [""; COUNT];
    for value in &mut values {
        *value = tokens.next(context)?;
    }
    tokens.finish(context)?;
    Ok(values)
}

struct TokenCursor<'a> {
    tokens: std::str::SplitAsciiWhitespace<'a>,
}

impl<'a> TokenCursor<'a> {
    fn new(line: &'a str) -> Self {
        Self {
            tokens: line.split_ascii_whitespace(),
        }
    }

    fn next(&mut self, context: &str) -> Result<&'a str, Diagnostic> {
        self.tokens
            .next()
            .ok_or_else(|| invalid_import(format!("missing token in {context}")))
    }

    fn finish(mut self, context: &str) -> Result<(), Diagnostic> {
        if self.tokens.next().is_some() {
            Err(invalid_import(format!(
                "unexpected trailing token in {context}",
            )))
        } else {
            Ok(())
        }
    }
}

fn parse_usize(value: &str, context: &str) -> Result<usize, Diagnostic> {
    value
        .parse()
        .map_err(|_| invalid_import(format!("invalid non-negative integer in {context}")))
}

fn parse_u64(value: &str, context: &str) -> Result<u64, Diagnostic> {
    value
        .parse()
        .map_err(|_| invalid_import(format!("invalid unsigned tag in {context}")))
}

fn parse_i32(value: &str, context: &str) -> Result<i32, Diagnostic> {
    value
        .parse()
        .map_err(|_| invalid_import(format!("invalid signed integer in {context}")))
}

fn parse_finite(value: &str, context: &str) -> Result<f64, Diagnostic> {
    let value = value
        .parse::<f64>()
        .map_err(|_| invalid_import(format!("invalid floating-point value in {context}")))?;
    if !value.is_finite() {
        return Err(invalid_import(format!("non-finite value in {context}")));
    }
    Ok(value)
}

fn decoded_coordinate(dimension: usize, x: f64, y: f64, z: f64) -> Result<Vec<f64>, Diagnostic> {
    match dimension {
        2 if z == 0.0 => fallible_vec_from_slice(&[x, y], "decoded vertex coordinate"),
        2 => Err(invalid_import(
            "two-dimensional import requires every node in the XY plane",
        )),
        3 => fallible_vec_from_slice(&[x, y, z], "decoded vertex coordinate"),
        _ => Err(invalid_import(
            "Gmsh simplex import supports spatial dimension two or three",
        )),
    }
}

fn checked_sum(values: &[usize], context: &str) -> Result<usize, Diagnostic> {
    values.iter().try_fold(0usize, |sum, &value| {
        sum.checked_add(value)
            .ok_or_else(|| invalid_import(format!("{context} overflows usize")))
    })
}

fn checked_next_index(index: usize, context: &str) -> Result<usize, Diagnostic> {
    index
        .checked_add(1)
        .ok_or_else(|| invalid_import(format!("{context} overflows usize")))
}

fn take_declared_line<'a>(
    lines: &[&'a str],
    cursor: &mut usize,
    context: &str,
) -> Result<&'a str, Diagnostic> {
    let line = lines
        .get(*cursor)
        .copied()
        .ok_or_else(|| invalid_import(format!("missing {context}")))?;
    *cursor = checked_next_index(*cursor, context)?;
    Ok(line)
}

fn fallible_vec<T>(capacity: usize, context: &str) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| invalid_import(format!("{context} exceeds available decoded memory")))?;
    Ok(values)
}

fn fallible_reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    context: &str,
) -> Result<(), Diagnostic> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| invalid_import(format!("{context} exceeds available decoded memory")))
}

fn fallible_push<T>(values: &mut Vec<T>, value: T, context: &str) -> Result<(), Diagnostic> {
    if values.len() == values.capacity() {
        values
            .try_reserve_exact(1)
            .map_err(|_| invalid_import(format!("{context} exceeds available decoded memory")))?;
    }
    values.push(value);
    Ok(())
}

fn fallible_vec_from_slice<T: Copy>(values: &[T], context: &str) -> Result<Vec<T>, Diagnostic> {
    let mut decoded = fallible_vec(values.len(), context)?;
    decoded.extend_from_slice(values);
    Ok(decoded)
}

fn fallible_set<T: Eq + Hash>(capacity: usize, context: &str) -> Result<HashSet<T>, Diagnostic> {
    let mut values = HashSet::new();
    values
        .try_reserve(capacity)
        .map_err(|_| invalid_import(format!("{context} exceeds available decoded memory")))?;
    Ok(values)
}

fn fallible_map<K: Eq + Hash, V>(
    capacity: usize,
    context: &str,
) -> Result<HashMap<K, V>, Diagnostic> {
    let mut values = HashMap::new();
    values
        .try_reserve(capacity)
        .map_err(|_| invalid_import(format!("{context} exceeds available decoded memory")))?;
    Ok(values)
}

fn invalid_import(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_MESH_IMPORT, message)
}

#[cfg(test)]
mod tests;
