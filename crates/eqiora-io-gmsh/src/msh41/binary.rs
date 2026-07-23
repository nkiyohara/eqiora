use std::collections::HashMap;

use eqiora_core::Diagnostic;

use super::{
    DecodedBudget, DecodedMesh, DecodedNodes, GmshImportLimits, MSH_VERSION, decoded_coordinate,
    exact_fields, fallible_map, fallible_push, fallible_reserve, fallible_set, fallible_vec,
    invalid_import, linear_simplex_type, parse_usize, strip_carriage_return,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Endian {
    Little,
    Big,
}

pub(super) fn parse(
    bytes: &[u8],
    dimension: usize,
    limits: GmshImportLimits,
    declared_size_t_size: usize,
    budget: &mut DecodedBudget,
) -> Result<DecodedMesh, Diagnostic> {
    let mut cursor = BinaryCursor::new(bytes);
    cursor.expect_line(b"$MeshFormat", "$MeshFormat section")?;
    let header = cursor.read_ascii_line("$MeshFormat header")?;
    let header = std::str::from_utf8(header)
        .map_err(|_| invalid_import("binary MSH format header must be ASCII"))?;
    let header = exact_fields::<3>(Some(header), "$MeshFormat header")?;
    if header[0] != MSH_VERSION
        || header[1] != "1"
        || parse_usize(header[2], "$MeshFormat data size")? != declared_size_t_size
    {
        return Err(invalid_import(
            "binary Gmsh import requires the MSH 4.1 header with file type one",
        ));
    }
    cursor.size_t_size = declared_size_t_size;
    cursor.endian = cursor.read_endian_marker()?;
    cursor.expect_line_break("binary endian marker")?;
    cursor.expect_line(b"$EndMeshFormat", "$EndMeshFormat delimiter")?;

    let entities = cursor.consume_line(b"$Entities")?;
    if entities {
        parse_entities(&mut cursor, dimension, limits, budget)?;
        cursor.end_section(b"$EndEntities", "$Entities section")?;
    }

    cursor.expect_line(b"$Nodes", "$Nodes section")?;
    let nodes = parse_nodes(&mut cursor, dimension, limits, budget)?;
    cursor.end_section(b"$EndNodes", "$Nodes section")?;

    cursor.expect_line(b"$Elements", "$Elements section")?;
    let cells = parse_elements(&mut cursor, dimension, limits, &nodes.vertex_by_tag, budget)?;
    cursor.end_section(b"$EndElements", "$Elements section")?;
    cursor.expect_end()?;

    Ok(DecodedMesh {
        vertices: nodes.vertices,
        cells,
    })
}

fn parse_entities(
    cursor: &mut BinaryCursor<'_>,
    dimension: usize,
    limits: GmshImportLimits,
    budget: &mut DecodedBudget,
) -> Result<(), Diagnostic> {
    let mut counts = [0_usize; 4];
    for count in &mut counts {
        *count = cursor.read_usize("$Entities count")?;
    }
    if counts
        .iter()
        .enumerate()
        .any(|(entity_dimension, &count)| entity_dimension > dimension && count != 0)
    {
        return Err(invalid_import(
            "$Entities contains geometry above the requested mesh dimension",
        ));
    }
    let total = counts.iter().try_fold(0_usize, |sum, &count| {
        sum.checked_add(count)
            .ok_or_else(|| invalid_import("$Entities count overflows usize"))
    })?;
    if total > limits.max_entities {
        return Err(invalid_import(
            "$Entities exceeds the configured entity limit",
        ));
    }
    budget.charge_entities(&counts)?;

    cursor.ensure_minimum_remaining(
        &[
            (counts[0], cursor.entity_record_size(0)?),
            (counts[1], cursor.entity_record_size(1)?),
            (counts[2], cursor.entity_record_size(2)?),
            (counts[3], cursor.entity_record_size(3)?),
        ],
        "$Entities declarations",
    )?;

    let mut tags = [
        fallible_set(counts[0], "$Entities point-tag storage")?,
        fallible_set(counts[1], "$Entities curve-tag storage")?,
        fallible_set(counts[2], "$Entities surface-tag storage")?,
        fallible_set(counts[3], "$Entities volume-tag storage")?,
    ];
    let mut references = 0_usize;
    for (entity_dimension, &count) in counts.iter().enumerate() {
        for _ in 0..count {
            let tag = cursor.read_i32("$Entities tag")?;
            if tag <= 0 || !tags[entity_dimension].insert(tag) {
                return Err(invalid_import(
                    "entity tags must be positive and unique per dimension",
                ));
            }
            let coordinate_count = if entity_dimension == 0 { 3 } else { 6 };
            for _ in 0..coordinate_count {
                cursor.read_finite("$Entities coordinate")?;
            }
            if cursor.read_usize("$Entities physical tag count")? != 0 {
                return Err(invalid_import(
                    "physical-group membership is outside the admitted import boundary",
                ));
            }
            if entity_dimension > 0 {
                let count = cursor.read_usize("$Entities boundary count")?;
                references = references
                    .checked_add(count)
                    .ok_or_else(|| invalid_import("$Entities reference count overflows usize"))?;
                if references > limits.max_entity_references {
                    return Err(invalid_import(
                        "$Entities exceeds the configured boundary-reference limit",
                    ));
                }
                budget.charge_entity_references(count)?;
                cursor.ensure_minimum_remaining(
                    &[(count, 4)],
                    "$Entities boundary-reference declaration",
                )?;
                for _ in 0..count {
                    if cursor.read_i32("$Entities boundary tag")? == 0 {
                        return Err(invalid_import("entity boundary tags must be nonzero"));
                    }
                }
            }
        }
    }
    Ok(())
}

fn parse_nodes(
    cursor: &mut BinaryCursor<'_>,
    dimension: usize,
    limits: GmshImportLimits,
    budget: &mut DecodedBudget,
) -> Result<DecodedNodes, Diagnostic> {
    let block_count = cursor.read_usize("$Nodes block count")?;
    let total_nodes = cursor.read_usize("$Nodes total count")?;
    let declared_min = cursor.read_size_t("$Nodes minimum tag")?;
    let declared_max = cursor.read_size_t("$Nodes maximum tag")?;
    if block_count == 0
        || total_nodes == 0
        || block_count > limits.max_node_blocks
        || total_nodes > limits.max_nodes
    {
        return Err(invalid_import(
            "$Nodes count is zero or exceeds its resource limit",
        ));
    }
    budget.charge_nodes(block_count, total_nodes, dimension)?;

    cursor.ensure_minimum_remaining(
        &[
            (block_count, cursor.node_block_header_size()?),
            (total_nodes, cursor.node_record_size()?),
        ],
        "$Nodes declarations",
    )?;

    let mut vertices = fallible_vec(total_nodes, "decoded vertex storage")?;
    let mut vertex_by_tag = fallible_map(total_nodes, "$Nodes tag-to-vertex storage")?;
    let mut observed_min = u64::MAX;
    let mut observed_max = 0_u64;
    let mut checked_total = 0_usize;
    for _ in 0..block_count {
        let entity_dim = cursor.read_i32("$Nodes entity dimension")?;
        let entity_tag = cursor.read_i32("$Nodes entity tag")?;
        let parametric = cursor.read_i32("$Nodes parametric flag")?;
        let count = cursor.read_usize("$Nodes block count")?;
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
        cursor.ensure_minimum_remaining(
            &[(count, cursor.node_record_size()?)],
            "$Nodes block declaration",
        )?;

        let block_start = vertices.len();
        for offset in 0..count {
            let tag = cursor.read_size_t("$Nodes node tag")?;
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
            let x = cursor.read_finite("$Nodes x coordinate")?;
            let y = cursor.read_finite("$Nodes y coordinate")?;
            let z = cursor.read_finite("$Nodes z coordinate")?;
            vertices.push(decoded_coordinate(dimension, x, y, z)?);
        }
    }

    if checked_total != total_nodes
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
    cursor: &mut BinaryCursor<'_>,
    dimension: usize,
    limits: GmshImportLimits,
    vertex_by_tag: &HashMap<u64, usize>,
    budget: &mut DecodedBudget,
) -> Result<Vec<Vec<usize>>, Diagnostic> {
    let block_count = cursor.read_usize("$Elements block count")?;
    let total_elements = cursor.read_usize("$Elements total count")?;
    let declared_min = cursor.read_size_t("$Elements minimum tag")?;
    let declared_max = cursor.read_size_t("$Elements maximum tag")?;
    if block_count == 0
        || total_elements == 0
        || block_count > limits.max_element_blocks
        || total_elements > limits.max_elements
    {
        return Err(invalid_import(
            "$Elements count is zero or exceeds its resource limit",
        ));
    }
    budget.charge_element_blocks(block_count)?;

    cursor.ensure_minimum_remaining(
        &[
            (block_count, cursor.element_block_header_size()?),
            (total_elements, cursor.minimum_element_record_size()?),
        ],
        "$Elements declarations",
    )?;

    let mut cells = Vec::new();
    let mut all_tags = fallible_set(total_elements, "$Elements unique-tag storage")?;
    let mut observed_min = u64::MAX;
    let mut observed_max = 0_u64;
    let mut checked_total = 0_usize;
    let mut top_dimensional_elements = 0_usize;
    for _ in 0..block_count {
        let entity_dim = cursor.read_i32("$Elements entity dimension")?;
        let entity_tag = cursor.read_i32("$Elements entity tag")?;
        let raw_element_type = cursor.read_i32("$Elements element type")?;
        let count = cursor.read_usize("$Elements block count")?;
        if entity_dim < 0 || entity_dim as usize > dimension || entity_tag <= 0 {
            return Err(invalid_import(
                "element blocks require an admitted entity dimension and positive tag",
            ));
        }
        let entity_dimension = entity_dim as usize;
        let element_type = u32::try_from(raw_element_type)
            .map_err(|_| invalid_import("MSH element type must be positive"))?;
        let expected_type = linear_simplex_type(entity_dimension);
        if element_type != expected_type {
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
        cursor.ensure_minimum_remaining(
            &[(count, cursor.element_record_size(entity_dimension)?)],
            "$Elements block declaration",
        )?;

        for _ in 0..count {
            let tag = cursor.read_size_t("$Elements element tag")?;
            if tag == 0 || !all_tags.insert(tag) {
                return Err(invalid_import("element tags must be positive and unique"));
            }
            observed_min = observed_min.min(tag);
            observed_max = observed_max.max(tag);
            let mut cell = if entity_dimension == dimension {
                Some(fallible_vec(
                    entity_dimension + 1,
                    "decoded simplex-connectivity storage",
                )?)
            } else {
                None
            };
            for _ in 0..=entity_dimension {
                let node_tag = cursor.read_size_t("$Elements node tag")?;
                if node_tag == 0 {
                    return Err(invalid_import("element node tags must be positive"));
                }
                let vertex = vertex_by_tag
                    .get(&node_tag)
                    .copied()
                    .ok_or_else(|| invalid_import("MSH element references an unknown node tag"))?;
                if let Some(cell) = &mut cell {
                    cell.push(vertex);
                }
            }
            if let Some(cell) = cell {
                fallible_push(&mut cells, cell, "decoded top-dimensional cell storage")?;
            }
        }
    }

    if checked_total != total_elements
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
    Ok(cells)
}

struct BinaryCursor<'a> {
    bytes: &'a [u8],
    position: usize,
    endian: Endian,
    size_t_size: usize,
}

impl<'a> BinaryCursor<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            position: 0,
            endian: Endian::Little,
            size_t_size: 0,
        }
    }

    fn read_endian_marker(&mut self) -> Result<Endian, Diagnostic> {
        let marker = self.take(4, "binary endian marker")?;
        match marker {
            [1, 0, 0, 0] => Ok(Endian::Little),
            [0, 0, 0, 1] => Ok(Endian::Big),
            _ => Err(invalid_import(
                "binary MSH endian marker must encode the integer one",
            )),
        }
    }

    fn encoded_size_t_size(&self) -> Result<usize, Diagnostic> {
        match self.size_t_size {
            size @ (4 | 8) => Ok(size),
            _ => Err(invalid_import(
                "binary MSH size_t width was not admitted by the format header",
            )),
        }
    }

    fn entity_record_size(&self, dimension: usize) -> Result<usize, Diagnostic> {
        let coordinate_bytes = if dimension == 0 { 3 * 8 } else { 6 * 8 };
        let count_fields = if dimension == 0 { 1 } else { 2 };
        let count_bytes = count_fields * self.encoded_size_t_size()?;
        4_usize
            .checked_add(coordinate_bytes)
            .and_then(|size| size.checked_add(count_bytes))
            .ok_or_else(|| invalid_import("$Entities minimum record size overflows usize"))
    }

    fn node_block_header_size(&self) -> Result<usize, Diagnostic> {
        self.encoded_size_t_size()?
            .checked_add(3 * 4)
            .ok_or_else(|| invalid_import("$Nodes block-header size overflows usize"))
    }

    fn node_record_size(&self) -> Result<usize, Diagnostic> {
        self.encoded_size_t_size()?
            .checked_add(3 * 8)
            .ok_or_else(|| invalid_import("$Nodes minimum record size overflows usize"))
    }

    fn element_block_header_size(&self) -> Result<usize, Diagnostic> {
        self.encoded_size_t_size()?
            .checked_add(3 * 4)
            .ok_or_else(|| invalid_import("$Elements block-header size overflows usize"))
    }

    fn minimum_element_record_size(&self) -> Result<usize, Diagnostic> {
        self.element_record_size(0)
    }

    fn element_record_size(&self, dimension: usize) -> Result<usize, Diagnostic> {
        let width = self.encoded_size_t_size()?;
        dimension
            .checked_add(2)
            .and_then(|fields| fields.checked_mul(width))
            .ok_or_else(|| invalid_import("$Elements minimum record size overflows usize"))
    }

    fn ensure_minimum_remaining(
        &self,
        terms: &[(usize, usize)],
        context: &str,
    ) -> Result<(), Diagnostic> {
        let required = terms.iter().try_fold(0_usize, |sum, &(count, width)| {
            count
                .checked_mul(width)
                .and_then(|bytes| sum.checked_add(bytes))
                .ok_or_else(|| invalid_import(format!("{context} byte budget overflows usize")))
        })?;
        let remaining = self.bytes.len().saturating_sub(self.position);
        if required > remaining {
            Err(invalid_import(format!(
                "{context} exceed the minimum records encodable in the remaining input",
            )))
        } else {
            Ok(())
        }
    }

    fn read_i32(&mut self, context: &str) -> Result<i32, Diagnostic> {
        let bytes: [u8; 4] = self
            .take(4, context)?
            .try_into()
            .expect("the bounded cursor returned four bytes");
        Ok(match self.endian {
            Endian::Little => i32::from_le_bytes(bytes),
            Endian::Big => i32::from_be_bytes(bytes),
        })
    }

    fn read_size_t(&mut self, context: &str) -> Result<u64, Diagnostic> {
        match self.size_t_size {
            4 => {
                let bytes: [u8; 4] = self
                    .take(4, context)?
                    .try_into()
                    .expect("the bounded cursor returned four bytes");
                Ok(match self.endian {
                    Endian::Little => u32::from_le_bytes(bytes),
                    Endian::Big => u32::from_be_bytes(bytes),
                }
                .into())
            }
            8 => {
                let bytes: [u8; 8] = self
                    .take(8, context)?
                    .try_into()
                    .expect("the bounded cursor returned eight bytes");
                Ok(match self.endian {
                    Endian::Little => u64::from_le_bytes(bytes),
                    Endian::Big => u64::from_be_bytes(bytes),
                })
            }
            _ => Err(invalid_import(
                "binary MSH size_t width was not admitted by the format header",
            )),
        }
    }

    fn read_usize(&mut self, context: &str) -> Result<usize, Diagnostic> {
        usize::try_from(self.read_size_t(context)?)
            .map_err(|_| invalid_import(format!("{context} exceeds local usize")))
    }

    fn read_finite(&mut self, context: &str) -> Result<f64, Diagnostic> {
        let bytes: [u8; 8] = self
            .take(8, context)?
            .try_into()
            .expect("the bounded cursor returned eight bytes");
        let value = match self.endian {
            Endian::Little => f64::from_le_bytes(bytes),
            Endian::Big => f64::from_be_bytes(bytes),
        };
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid_import(format!("non-finite value in {context}")))
        }
    }

    fn take(&mut self, count: usize, context: &str) -> Result<&'a [u8], Diagnostic> {
        let end = self
            .position
            .checked_add(count)
            .ok_or_else(|| invalid_import(format!("{context} byte range overflows usize")))?;
        let bytes = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| invalid_import(format!("truncated binary data in {context}")))?;
        self.position = end;
        Ok(bytes)
    }

    fn read_ascii_line(&mut self, context: &str) -> Result<&'a [u8], Diagnostic> {
        let tail = self
            .bytes
            .get(self.position..)
            .ok_or_else(|| invalid_import(format!("missing {context}")))?;
        let length = tail
            .iter()
            .position(|&byte| byte == b'\n')
            .ok_or_else(|| invalid_import(format!("unterminated {context}")))?;
        let line = strip_carriage_return(&tail[..length]);
        self.position = self
            .position
            .checked_add(length + 1)
            .ok_or_else(|| invalid_import(format!("{context} position overflows usize")))?;
        Ok(line)
    }

    fn expect_line(&mut self, expected: &[u8], context: &str) -> Result<(), Diagnostic> {
        if self.read_ascii_line(context)? == expected {
            Ok(())
        } else {
            Err(invalid_import(format!("invalid {context}")))
        }
    }

    fn consume_line(&mut self, expected: &[u8]) -> Result<bool, Diagnostic> {
        let original = self.position;
        let line = self.read_ascii_line("MSH section delimiter")?;
        if line == expected {
            Ok(true)
        } else {
            self.position = original;
            Ok(false)
        }
    }

    fn expect_line_break(&mut self, context: &str) -> Result<(), Diagnostic> {
        if self.bytes.get(self.position) == Some(&b'\n') {
            self.position += 1;
            return Ok(());
        }
        if self
            .bytes
            .get(self.position..self.position.saturating_add(2))
            == Some(b"\r\n")
        {
            self.position += 2;
            return Ok(());
        }
        Err(invalid_import(format!(
            "{context} must be followed by a line break",
        )))
    }

    fn end_section(&mut self, delimiter: &[u8], context: &str) -> Result<(), Diagnostic> {
        self.expect_line_break(context)?;
        self.expect_line(delimiter, context)
    }

    fn expect_end(&self) -> Result<(), Diagnostic> {
        if self.bytes[self.position..]
            .iter()
            .all(|byte| byte.is_ascii_whitespace())
        {
            Ok(())
        } else {
            Err(invalid_import(
                "binary MSH input contains an unsupported or trailing section",
            ))
        }
    }
}
