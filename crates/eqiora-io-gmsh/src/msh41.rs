use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::panic::{AssertUnwindSafe, catch_unwind};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_meshing::{MeshQualityGate, SimplicialMesh};
use mshio::mshfile::ElementType;

mod binary;
mod budget;

use budget::DecodedBudget;

const MSH_VERSION: &str = "4.1";

/// Decoded-work and input-size policy applied before the third-party parser.
///
/// Every declared loop is charged to one of these semantic count budgets.
/// Binary counts must also fit the minimum records encodable in the remaining
/// input. Aggregate decoded-byte and work budgets conservatively cover
/// preflight state, worst-case sparse parser materialization, canonical output,
/// and simplex topology closure; declaration-sized importer storage also uses
/// fallible reservation. The byte budget is a deterministic logical account,
/// not a promise of exact allocator RSS.
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
    /// Maximum aggregate logical bytes for preflight, parser, and canonical mesh state.
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

    /// Resource policy applied before dependency parsing.
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
        if bytes.len() > self.limits.max_bytes {
            return Err(invalid_import(
                "Gmsh input exceeds the configured byte limit",
            ));
        }
        let encoding = declared_encoding(bytes)?;
        let mut decoded_budget = DecodedBudget::new(self.limits);
        let preflight = match encoding {
            InputEncoding::Ascii => {
                let text = std::str::from_utf8(bytes)
                    .map_err(|_| invalid_import("ASCII MSH 4.1 input must be valid UTF-8"))?;
                Preflight::parse(text, self.dimension, self.limits, &mut decoded_budget)?
            }
            InputEncoding::Binary { size_t_size } => binary::parse(
                bytes,
                self.dimension,
                self.limits,
                size_t_size,
                &mut decoded_budget,
            )?,
        };

        let parsed = catch_unwind(AssertUnwindSafe(|| mshio::parse_msh_bytes(bytes)))
            .map_err(|_| invalid_import("the isolated MSH parser panicked on malformed input"))?
            .map_err(|_| invalid_import("the isolated MSH parser rejected the input syntax"))?;
        if parsed.header.version != 4.1
            || parsed.header.file_type != encoding.file_type()
            || parsed.header.size_t_size != encoding.size_t_size()
            || parsed.header.int_size != 4
            || parsed.header.float_size != 8
            || parsed.header.endianness.is_some() != encoding.is_binary()
        {
            return Err(invalid_import(
                "the parsed MSH header differs from the bounded format preflight",
            ));
        }

        let nodes = parsed
            .data
            .nodes
            .ok_or_else(|| invalid_import("MSH input has no parsed node section"))?;
        if nodes.node_blocks.len() != preflight.node_blocks.len()
            || nodes.num_nodes != preflight.total_nodes as u64
        {
            return Err(invalid_import(
                "parsed node structure differs from the bounded preflight",
            ));
        }

        let mut vertices = fallible_vec(preflight.total_nodes, "decoded vertex storage")?;
        let mut node_tags = fallible_vec(preflight.total_nodes, "decoded node-tag storage")?;
        for (parsed_block, checked_block) in nodes.node_blocks.iter().zip(&preflight.node_blocks) {
            if parsed_block.entity_dim != checked_block.entity_dim
                || parsed_block.entity_tag != checked_block.entity_tag
                || parsed_block.parametric
                || parsed_block.nodes.len() != checked_block.tags.len()
            {
                return Err(invalid_import(
                    "parsed node block differs from the bounded preflight",
                ));
            }
            for (node, &tag) in parsed_block.nodes.iter().zip(&checked_block.tags) {
                let coordinate = match self.dimension {
                    2 if node.z == 0.0 => {
                        fallible_vec_from_slice(&[node.x, node.y], "decoded vertex coordinate")?
                    }
                    2 => {
                        return Err(invalid_import(
                            "two-dimensional import requires every node in the XY plane",
                        ));
                    }
                    3 => fallible_vec_from_slice(
                        &[node.x, node.y, node.z],
                        "decoded vertex coordinate",
                    )?,
                    _ => unreachable!("constructor validates importer dimension"),
                };
                if coordinate.iter().any(|value| !value.is_finite()) {
                    return Err(invalid_import("MSH node coordinates must be finite"));
                }
                node_tags.push(tag);
                vertices.push(coordinate);
            }
        }

        let mut tag_to_vertex = fallible_map(node_tags.len(), "decoded node lookup")?;
        for (index, &tag) in node_tags.iter().enumerate() {
            tag_to_vertex.insert(tag, index);
        }
        if tag_to_vertex.len() != node_tags.len() {
            return Err(invalid_import("MSH node tags must be unique"));
        }

        let elements = parsed
            .data
            .elements
            .ok_or_else(|| invalid_import("MSH input has no parsed element section"))?;
        if elements.element_blocks.len() != preflight.element_blocks.len()
            || elements.num_elements != preflight.total_elements as u64
        {
            return Err(invalid_import(
                "parsed element structure differs from the bounded preflight",
            ));
        }

        let expected_type = match self.dimension {
            2 => ElementType::Tri3,
            3 => ElementType::Tet4,
            _ => unreachable!("constructor validates importer dimension"),
        };
        let mut cells = fallible_vec(preflight.top_dimensional_elements, "decoded cell storage")?;
        for (parsed_block, checked_block) in elements
            .element_blocks
            .iter()
            .zip(&preflight.element_blocks)
        {
            if parsed_block.entity_dim != checked_block.entity_dim
                || parsed_block.entity_tag != checked_block.entity_tag
                || parsed_block.element_type as u32 != checked_block.element_type
                || parsed_block.elements.len() != checked_block.element_tags.len()
            {
                return Err(invalid_import(
                    "parsed element block differs from the bounded preflight",
                ));
            }
            for (element, &checked_tag) in parsed_block
                .elements
                .iter()
                .zip(&checked_block.element_tags)
            {
                if element.element_tag != checked_tag {
                    return Err(invalid_import(
                        "parsed element tags differ from the bounded preflight",
                    ));
                }
            }
            if parsed_block.entity_dim < self.dimension as i32 {
                continue;
            }
            if parsed_block.entity_dim != self.dimension as i32
                || parsed_block.element_type != expected_type
            {
                return Err(invalid_import(
                    "MSH top-dimensional cells must be linear simplices of the requested dimension",
                ));
            }
            for element in &parsed_block.elements {
                let mut cell =
                    fallible_vec(element.nodes.len(), "decoded simplex-connectivity storage")?;
                for tag in &element.nodes {
                    cell.push(tag_to_vertex.get(tag).copied().ok_or_else(|| {
                        invalid_import("MSH element references an unknown node tag")
                    })?);
                }
                cells.push(cell);
            }
        }
        if cells.is_empty() {
            return Err(invalid_import(
                "MSH input contains no admitted top-dimensional simplex cells",
            ));
        }
        if cells.len() != preflight.top_dimensional_elements {
            return Err(invalid_import(
                "parsed top-dimensional cell count differs from the bounded preflight",
            ));
        }

        SimplicialMesh::new(self.dimension, vertices, cells, self.quality_gate)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputEncoding {
    Ascii,
    Binary { size_t_size: usize },
}

impl InputEncoding {
    const fn file_type(self) -> i32 {
        match self {
            Self::Ascii => 0,
            Self::Binary { .. } => 1,
        }
    }

    const fn size_t_size(self) -> usize {
        match self {
            Self::Ascii => 8,
            Self::Binary { size_t_size } => size_t_size,
        }
    }

    const fn is_binary(self) -> bool {
        matches!(self, Self::Binary { .. })
    }
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
struct Preflight {
    node_blocks: Vec<NodeBlock>,
    element_blocks: Vec<ElementBlock>,
    total_nodes: usize,
    total_elements: usize,
    top_dimensional_elements: usize,
}

#[derive(Debug)]
struct NodeBlock {
    entity_dim: i32,
    entity_tag: i32,
    tags: Vec<u64>,
}

#[derive(Debug)]
struct ElementBlock {
    entity_dim: i32,
    entity_tag: i32,
    element_type: u32,
    element_tags: Vec<u64>,
}

impl Preflight {
    fn parse(
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
        let (node_blocks, total_nodes) =
            parse_nodes(sections.required("Nodes")?, dimension, limits, budget)?;
        let (element_blocks, total_elements, top_dimensional_elements) =
            parse_elements(sections.required("Elements")?, dimension, limits, budget)?;
        Ok(Self {
            node_blocks,
            element_blocks,
            total_nodes,
            total_elements,
            top_dimensional_elements,
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
) -> Result<(Vec<NodeBlock>, usize), Diagnostic> {
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
    let mut blocks = fallible_vec(block_count, "$Nodes block storage")?;
    let mut all_tags = fallible_set(total_nodes, "$Nodes unique-tag storage")?;
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
        let mut tags = fallible_vec(count, "$Nodes block-tag storage")?;
        for _ in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Nodes node tag")?;
            let [value] = exact_fields::<1>(Some(line), "$Nodes node tag")?;
            let tag = parse_u64(value, "$Nodes node tag")?;
            if tag == 0 || !all_tags.insert(tag) {
                return Err(invalid_import("node tags must be positive and unique"));
            }
            observed_min = observed_min.min(tag);
            observed_max = observed_max.max(tag);
            tags.push(tag);
        }
        for _ in 0..count {
            let line = take_declared_line(lines, &mut line_index, "$Nodes coordinate")?;
            let coordinate = exact_fields::<3>(Some(line), "$Nodes coordinate")?;
            for value in coordinate {
                parse_finite(value, "$Nodes coordinate")?;
            }
        }
        blocks.push(NodeBlock {
            entity_dim,
            entity_tag,
            tags,
        });
    }
    if line_index != lines.len()
        || checked_total != total_nodes
        || all_tags.len() != total_nodes
        || observed_min != declared_min
        || observed_max != declared_max
    {
        return Err(invalid_import(
            "$Nodes records disagree with declared totals or tag bounds",
        ));
    }
    Ok((blocks, total_nodes))
}

fn parse_elements(
    lines: &[&str],
    dimension: usize,
    limits: GmshImportLimits,
    budget: &mut DecodedBudget,
) -> Result<(Vec<ElementBlock>, usize, usize), Diagnostic> {
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
    let mut blocks = fallible_vec(block_count, "$Elements block storage")?;
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
        }
        let mut element_tags = fallible_vec(count, "$Elements block-tag storage")?;
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
            for _ in 0..=entity_dimension {
                if parse_u64(tokens.next("$Elements node tag")?, "$Elements node tag")? == 0 {
                    return Err(invalid_import("element node tags must be positive"));
                }
            }
            tokens.finish("$Elements record")?;
            element_tags.push(tag);
        }
        blocks.push(ElementBlock {
            entity_dim,
            entity_tag,
            element_type,
            element_tags,
        });
    }
    if line_index != lines.len()
        || checked_total != total_elements
        || all_tags.len() != total_elements
        || observed_min != declared_min
        || observed_max != declared_max
    {
        return Err(invalid_import(
            "$Elements records disagree with declared totals or tag bounds",
        ));
    }
    Ok((blocks, total_elements, top_dimensional_elements))
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
mod tests {
    use std::panic::catch_unwind;

    use eqiora_core::diagnostic::codes;
    use eqiora_meshing::{MeshQualityGate, MeshTopology};

    use super::{GmshImportLimits, GmshSimplexImporter, fallible_map, fallible_set, fallible_vec};

    #[derive(Clone, Copy)]
    enum TestEndian {
        Little,
        Big,
    }

    const TRIANGLES: &str = "$MeshFormat\n4.1 0 8\n$EndMeshFormat\n\
$Entities\n4 4 1 0\n1 0 0 0 0\n2 1 0 0 0\n3 1 1 0 0\n4 0 1 0 0\n1 0 0 0 1 0 0 0 2 1 -2\n2 1 0 0 1 1 0 0 2 2 -3\n3 0 1 0 1 1 0 0 2 3 -4\n4 0 0 0 0 1 0 0 2 4 -1\n1 0 0 0 1 1 0 0 4 1 2 3 4\n$EndEntities\n\
$Nodes\n2 5 10 50\n1 1 0 2\n10\n20\n0 0 0\n1 0 0\n2 1 0 3\n30\n40\n50\n1 1 0\n0 1 0\n0.5 0.5 0\n$EndNodes\n\
$Elements\n2 8 101 204\n1 1 1 4\n101 10 20\n102 20 30\n103 30 40\n104 40 10\n2 1 2 4\n201 10 20 50\n202 20 30 50\n203 30 40 50\n204 40 10 50\n$EndElements\n";

    const TETRAHEDRON: &str = "$MeshFormat\n4.1 0 8\n$EndMeshFormat\n\
$Nodes\n1 4 1 4\n3 1 0 4\n1\n2\n3\n4\n0 0 0\n1 0 0\n0 1 0\n0 0 1\n$EndNodes\n\
$Elements\n1 1 1 1\n3 1 4 1\n1 1 2 3 4\n$EndElements\n";

    fn importer() -> GmshSimplexImporter {
        GmshSimplexImporter::new(
            2,
            MeshQualityGate::new(0.5).unwrap(),
            GmshImportLimits::default(),
        )
        .unwrap()
    }

    fn tetrahedron_importer() -> GmshSimplexImporter {
        GmshSimplexImporter::new(
            3,
            MeshQualityGate::new(0.1).unwrap(),
            GmshImportLimits::default(),
        )
        .unwrap()
    }

    fn binary_tetrahedron(endian: TestEndian, size_t_size: usize, element_type: i32) -> Vec<u8> {
        binary_tetrahedron_with_ignored_points(endian, size_t_size, element_type, 0)
    }

    fn binary_tetrahedron_with_ignored_points(
        endian: TestEndian,
        size_t_size: usize,
        element_type: i32,
        ignored_points: usize,
    ) -> Vec<u8> {
        let mut bytes = format!("$MeshFormat\n4.1 1 {size_t_size}\n").into_bytes();
        write_i32(&mut bytes, 1, endian);
        bytes.extend_from_slice(b"\n$EndMeshFormat\n$Nodes\n");
        for value in [1, 4, 1, 4] {
            write_size_t(&mut bytes, value, size_t_size, endian);
        }
        for value in [3, 1, 0] {
            write_i32(&mut bytes, value, endian);
        }
        write_size_t(&mut bytes, 4, size_t_size, endian);
        for tag in 1..=4 {
            write_size_t(&mut bytes, tag, size_t_size, endian);
        }
        for coordinate in [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0] {
            write_f64(&mut bytes, coordinate, endian);
        }
        bytes.extend_from_slice(b"\n$EndNodes\n$Elements\n");
        let ignored_points = u64::try_from(ignored_points).unwrap();
        let block_count = if ignored_points == 0 { 1 } else { 2 };
        let element_count = ignored_points + 1;
        for value in [block_count, element_count, 1, element_count] {
            write_size_t(&mut bytes, value, size_t_size, endian);
        }
        if ignored_points != 0 {
            for value in [0, 1, 15] {
                write_i32(&mut bytes, value, endian);
            }
            write_size_t(&mut bytes, ignored_points, size_t_size, endian);
            for tag in 1..=ignored_points {
                write_size_t(&mut bytes, tag, size_t_size, endian);
                write_size_t(&mut bytes, 1, size_t_size, endian);
            }
        }
        for value in [3, 1, element_type] {
            write_i32(&mut bytes, value, endian);
        }
        write_size_t(&mut bytes, 1, size_t_size, endian);
        write_size_t(&mut bytes, element_count, size_t_size, endian);
        for value in 1..=4 {
            write_size_t(&mut bytes, value, size_t_size, endian);
        }
        bytes.extend_from_slice(b"\n$EndElements\n");
        bytes
    }

    fn write_i32(bytes: &mut Vec<u8>, value: i32, endian: TestEndian) {
        let encoded = match endian {
            TestEndian::Little => value.to_le_bytes(),
            TestEndian::Big => value.to_be_bytes(),
        };
        bytes.extend_from_slice(&encoded);
    }

    fn write_size_t(bytes: &mut Vec<u8>, value: u64, width: usize, endian: TestEndian) {
        match width {
            4 => {
                let value = value as u32;
                let encoded = match endian {
                    TestEndian::Little => value.to_le_bytes(),
                    TestEndian::Big => value.to_be_bytes(),
                };
                bytes.extend_from_slice(&encoded);
            }
            8 => {
                let encoded = match endian {
                    TestEndian::Little => value.to_le_bytes(),
                    TestEndian::Big => value.to_be_bytes(),
                };
                bytes.extend_from_slice(&encoded);
            }
            _ => panic!("test writer requires a four- or eight-byte size_t"),
        }
    }

    fn overwrite_size_t(
        bytes: &mut [u8],
        offset: usize,
        value: u64,
        width: usize,
        endian: TestEndian,
    ) {
        let mut encoded = Vec::new();
        write_size_t(&mut encoded, value, width, endian);
        bytes[offset..offset + width].copy_from_slice(&encoded);
    }

    fn write_f64(bytes: &mut Vec<u8>, value: f64, endian: TestEndian) {
        let encoded = match endian {
            TestEndian::Little => value.to_le_bytes(),
            TestEndian::Big => value.to_be_bytes(),
        };
        bytes.extend_from_slice(&encoded);
    }

    fn replace_once(source: &[u8], needle: &[u8], replacement: &[u8]) -> Vec<u8> {
        let start = source
            .windows(needle.len())
            .position(|window| window == needle)
            .expect("test fixture contains replacement target");
        let mut result = Vec::with_capacity(source.len() - needle.len() + replacement.len());
        result.extend_from_slice(&source[..start]);
        result.extend_from_slice(replacement);
        result.extend_from_slice(&source[start + needle.len()..]);
        result
    }

    #[test]
    fn imports_sparse_multiblock_triangles_and_ignores_boundary_elements() {
        let mesh = importer().import_bytes(TRIANGLES.as_bytes()).unwrap();
        assert_eq!(mesh.topological_dimension(), 2);
        assert_eq!(mesh.vertices().len(), 5);
        assert_eq!(mesh.cells().len(), 4);
        assert_eq!(mesh.cells()[3], [3, 0, 4]);
        assert!(mesh.quality_report().minimum_mean_ratio() >= 0.5);
    }

    #[test]
    fn imports_one_positive_tetrahedron() {
        let mesh = tetrahedron_importer()
            .import_bytes(TETRAHEDRON.as_bytes())
            .unwrap();
        assert_eq!(mesh.topological_dimension(), 3);
        assert_eq!(mesh.cells(), &[vec![0, 1, 2, 3]]);
    }

    #[test]
    fn binary_endianness_and_size_t_width_are_representation_only() {
        let ascii = tetrahedron_importer()
            .import_bytes(TETRAHEDRON.as_bytes())
            .unwrap();
        for endian in [TestEndian::Little, TestEndian::Big] {
            for width in [4, 8] {
                let binary = binary_tetrahedron(endian, width, 4);
                assert_eq!(tetrahedron_importer().import_bytes(&binary).unwrap(), ascii);
            }
        }
    }

    #[test]
    fn every_truncated_binary_representation_prefix_fails_closed() {
        for endian in [TestEndian::Little, TestEndian::Big] {
            for width in [4, 8] {
                let binary = binary_tetrahedron(endian, width, 4);
                for end in 0..binary.len() {
                    assert_eq!(
                        tetrahedron_importer()
                            .import_bytes(&binary[..end])
                            .unwrap_err()
                            .code(),
                        codes::INVALID_MESH_IMPORT,
                        "{width}-byte representation prefix ending at byte {end} was unexpectedly admitted",
                    );
                }
            }
        }
    }

    #[test]
    fn binary_count_budgets_are_inclusive_and_extreme_limits_do_not_panic() {
        let valid = binary_tetrahedron(TestEndian::Little, 8, 4);
        let exact_limits = GmshImportLimits {
            max_bytes: valid.len(),
            max_entities: 1,
            max_entity_references: 1,
            max_node_blocks: 1,
            max_element_blocks: 1,
            max_nodes: 4,
            max_elements: 1,
            max_ignored_elements: 1,
            max_decoded_bytes: usize::MAX,
            max_decoded_work: usize::MAX,
        };
        let exact =
            GmshSimplexImporter::new(3, MeshQualityGate::new(0.1).unwrap(), exact_limits).unwrap();
        assert_eq!(exact.import_bytes(&valid).unwrap().cells().len(), 1);

        let mut forged = valid;
        let node_header = forged
            .windows(b"$Nodes\n".len())
            .position(|window| window == b"$Nodes\n")
            .unwrap()
            + b"$Nodes\n".len();
        overwrite_size_t(&mut forged, node_header, u64::MAX, 8, TestEndian::Little);
        let extreme_limits = GmshImportLimits {
            max_bytes: usize::MAX,
            max_entities: usize::MAX,
            max_entity_references: usize::MAX,
            max_node_blocks: usize::MAX,
            max_element_blocks: usize::MAX,
            max_nodes: usize::MAX,
            max_elements: usize::MAX,
            max_ignored_elements: usize::MAX,
            max_decoded_bytes: usize::MAX,
            max_decoded_work: usize::MAX,
        };
        let extreme =
            GmshSimplexImporter::new(3, MeshQualityGate::new(0.1).unwrap(), extreme_limits)
                .unwrap();
        let outcome = catch_unwind(|| extreme.import_bytes(&forged));
        assert_eq!(
            outcome
                .expect("forged declaration must not panic")
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT,
        );
    }

    #[test]
    fn aggregate_decoded_budgets_and_ignored_elements_fail_before_materialization() {
        let valid = binary_tetrahedron(TestEndian::Little, 8, 4);
        for limits in [
            GmshImportLimits {
                max_decoded_bytes: 1,
                ..GmshImportLimits::default()
            },
            GmshImportLimits {
                max_decoded_work: 1,
                ..GmshImportLimits::default()
            },
        ] {
            let bounded =
                GmshSimplexImporter::new(3, MeshQualityGate::new(0.1).unwrap(), limits).unwrap();
            assert_eq!(
                bounded.import_bytes(&valid).unwrap_err().code(),
                codes::INVALID_MESH_IMPORT,
            );
        }

        let default_limits = GmshImportLimits::default();
        let ignored = default_limits.max_ignored_elements + 1;
        let padded = binary_tetrahedron_with_ignored_points(TestEndian::Little, 8, 4, ignored);
        assert!(padded.len() < default_limits.max_bytes);
        assert_eq!(
            tetrahedron_importer()
                .import_bytes(&padded)
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT,
        );

        let admitted_ignored = 64;
        let admitted =
            binary_tetrahedron_with_ignored_points(TestEndian::Little, 8, 4, admitted_ignored);
        let explicit = GmshSimplexImporter::new(
            3,
            MeshQualityGate::new(0.1).unwrap(),
            GmshImportLimits {
                max_ignored_elements: admitted_ignored,
                ..GmshImportLimits::default()
            },
        )
        .unwrap();
        assert_eq!(explicit.import_bytes(&admitted).unwrap().cells().len(), 1);
    }

    #[test]
    fn ascii_extreme_declarations_never_panic_under_extreme_public_limits() {
        let maximum = usize::MAX.to_string();
        let forged_boundary = TRIANGLES.replacen(
            "1 0 0 0 1 0 0 0 2 1 -2",
            &format!("1 0 0 0 1 0 0 0 {maximum}"),
            1,
        );
        let forged_entities = TRIANGLES.replacen("4 4 1 0", &format!("{maximum} 4 1 0"), 1);
        let forged_nodes = TRIANGLES.replacen("2 5 10 50", &format!("2 {maximum} 10 50"), 1);
        let forged_elements = TRIANGLES.replacen("2 8 101 204", &format!("2 {maximum} 101 204"), 1);
        let importer = GmshSimplexImporter::new(
            2,
            MeshQualityGate::new(0.1).unwrap(),
            GmshImportLimits {
                max_bytes: usize::MAX,
                max_entities: usize::MAX,
                max_entity_references: usize::MAX,
                max_node_blocks: usize::MAX,
                max_element_blocks: usize::MAX,
                max_nodes: usize::MAX,
                max_elements: usize::MAX,
                max_ignored_elements: usize::MAX,
                max_decoded_bytes: usize::MAX,
                max_decoded_work: usize::MAX,
            },
        )
        .unwrap();
        for forged in [
            forged_boundary,
            forged_entities,
            forged_nodes,
            forged_elements,
        ] {
            let outcome = catch_unwind(|| importer.import_bytes(forged.as_bytes()));
            assert_eq!(
                outcome
                    .expect("extreme ASCII declaration must not panic")
                    .unwrap_err()
                    .code(),
                codes::INVALID_MESH_IMPORT,
            );
        }
    }

    #[test]
    fn token_dense_ascii_never_allocates_a_token_scratch_vector() {
        let token_count = 100_000_usize;
        let mut dense_header = String::from("$MeshFormat\n4.1 0 8");
        for _ in 0..token_count {
            dense_header.push_str(" 0");
        }
        dense_header.push_str("\n$EndMeshFormat\n");

        let mut dense_entity = String::from("1 0 0 0 1 0 0 0 ");
        dense_entity.push_str(&token_count.to_string());
        for _ in 0..token_count {
            dense_entity.push_str(" 1");
        }
        let dense_entities = TRIANGLES.replacen("1 0 0 0 1 0 0 0 2 1 -2", &dense_entity, 1);

        let limits = GmshImportLimits {
            max_decoded_bytes: 64 * 1024,
            ..GmshImportLimits::default()
        };
        let importer =
            GmshSimplexImporter::new(2, MeshQualityGate::new(0.1).unwrap(), limits).unwrap();
        for (dense, expected_budget_rejection) in [(dense_header, false), (dense_entities, true)] {
            assert!(dense.len() <= limits.max_bytes);
            let outcome = catch_unwind(|| importer.import_bytes(dense.as_bytes()));
            let diagnostic = outcome
                .expect("token-dense ASCII must not panic")
                .unwrap_err();
            assert_eq!(diagnostic.code(), codes::INVALID_MESH_IMPORT);
            if expected_budget_rejection {
                assert!(
                    diagnostic
                        .message()
                        .contains("aggregate decoded-byte budget")
                );
            }
        }
    }

    #[test]
    fn impossible_importer_owned_reservations_are_diagnostics() {
        assert_eq!(
            fallible_vec::<u8>(usize::MAX, "test vector")
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT,
        );
        assert_eq!(
            fallible_set::<u64>(usize::MAX, "test set")
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT,
        );
        assert_eq!(
            fallible_map::<u64, usize>(usize::MAX, "test map")
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT,
        );
    }

    #[test]
    fn binary_header_counts_sections_and_element_families_fail_closed() {
        let valid = binary_tetrahedron(TestEndian::Little, 8, 4);

        let mut invalid_marker = valid.clone();
        let marker = b"$MeshFormat\n4.1 1 8\n".len();
        invalid_marker[marker..marker + 4].copy_from_slice(&2_i32.to_le_bytes());

        let mut excessive_nodes = valid.clone();
        let node_header = excessive_nodes
            .windows(b"$Nodes\n".len())
            .position(|window| window == b"$Nodes\n")
            .unwrap()
            + b"$Nodes\n".len();
        excessive_nodes[node_header + 8..node_header + 16].copy_from_slice(&u64::MAX.to_le_bytes());

        let mut result_section = valid.clone();
        result_section.extend_from_slice(b"$NodeData\n$EndNodeData\n");

        for rejected in [
            invalid_marker,
            excessive_nodes,
            result_section,
            binary_tetrahedron(TestEndian::Little, 8, 11),
        ] {
            assert_eq!(
                tetrahedron_importer()
                    .import_bytes(&rejected)
                    .unwrap_err()
                    .code(),
                codes::INVALID_MESH_IMPORT,
            );
        }

        for header in ["4.1 1 1", "4.1 1 2", "4.1 1 16"] {
            let rejected = replace_once(&valid, b"4.1 1 8", header.as_bytes());
            assert_eq!(
                tetrahedron_importer()
                    .import_bytes(&rejected)
                    .unwrap_err()
                    .code(),
                codes::INVALID_MESH_IMPORT,
            );
        }
    }

    #[test]
    fn resource_version_and_semantic_boundaries_fail_closed() {
        let too_small = GmshSimplexImporter::new(
            2,
            MeshQualityGate::new(0.1).unwrap(),
            GmshImportLimits {
                max_bytes: 8,
                ..GmshImportLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            too_small
                .import_bytes(TRIANGLES.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );

        for rejected in [
            TRIANGLES.replacen("4.1 0 8", "4.1 1 8", 1),
            TRIANGLES.replacen("4.1 0 8", "2.2 0 8", 1),
            TRIANGLES.replacen("1 1 0 2", "1 1 1 2", 1),
            TRIANGLES.replacen("0.5 0.5 0", "0.5 0.5 0.25", 1),
            TRIANGLES.replacen("2 1 2 4", "2 1 3 4", 1),
        ] {
            assert_eq!(
                importer()
                    .import_bytes(rejected.as_bytes())
                    .unwrap_err()
                    .code(),
                codes::INVALID_MESH_IMPORT
            );
        }
    }

    #[test]
    fn inconsistent_tags_references_orientation_and_quality_are_rejected() {
        let duplicate = TRIANGLES.replacen("20\n0 0 0", "10\n0 0 0", 1);
        assert_eq!(
            importer()
                .import_bytes(duplicate.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );

        let missing = TRIANGLES.replacen("201 10 20 50", "201 10 20 99", 1);
        assert_eq!(
            importer()
                .import_bytes(missing.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );

        let inverted = TRIANGLES.replacen("201 10 20 50", "201 20 10 50", 1);
        assert_eq!(
            importer()
                .import_bytes(inverted.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH
        );

        let strict = GmshSimplexImporter::new(
            2,
            MeshQualityGate::new(0.99).unwrap(),
            GmshImportLimits::default(),
        )
        .unwrap();
        assert_eq!(
            strict
                .import_bytes(TRIANGLES.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH
        );
    }

    #[test]
    fn count_limits_and_malformed_sections_are_rejected_before_parsing() {
        let node_limited = GmshSimplexImporter::new(
            2,
            MeshQualityGate::new(0.1).unwrap(),
            GmshImportLimits {
                max_nodes: 4,
                ..GmshImportLimits::default()
            },
        )
        .unwrap();
        assert_eq!(
            node_limited
                .import_bytes(TRIANGLES.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );
        let malformed = TRIANGLES.replace("$EndNodes", "$EndWrong");
        assert_eq!(
            importer()
                .import_bytes(malformed.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );

        let forged_block_count = TRIANGLES.replacen("1 1 0 2", "1 1 0 999999999", 1);
        assert_eq!(
            importer()
                .import_bytes(forged_block_count.as_bytes())
                .unwrap_err()
                .code(),
            codes::INVALID_MESH_IMPORT
        );
    }
}
