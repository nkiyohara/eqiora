//! Exact byte-identical sealed E1 input and its contract-frozen selectors.
//!
//! The embedded bytes are the precommitted `#407` sealed input. Nothing here
//! authors a physical value, coefficient, topology, tolerance, or predicate:
//! every consumed datum is read from those bytes after their SHA-256 identity
//! is verified, and the contract-frozen JSON selectors are the only paths used.

use std::collections::BTreeMap;

use eqiora_core::Diagnostic;
use eqiora_meshing::MeshQualityGate;

use super::{
    StokesDissipationBodyCorrespondence2d, StokesDissipationBoundaryFacetSource2d,
    StokesDissipationCellRecord2d, StokesDissipationTopologyRole2d,
    StokesDissipationTopologySource2d, StokesDissipationVertexRecord2d, invalid,
};

/// The exact sealed `#407` input, embedded byte-identically.
const SEALED_INPUT_BYTES: &[u8] = include_bytes!("e1-sealed-inputs-v1.json");

/// `da3223d51caf11f6e627540f9284c2bb307518f7d87a62e2897a9f3732bbf620`.
const SEALED_INPUT_SHA256: [u8; 32] = [
    0xda, 0x32, 0x23, 0xd5, 0x1c, 0xaf, 0x11, 0xf6, 0xe6, 0x27, 0x54, 0x0f, 0x92, 0x84, 0xc2, 0xbb,
    0x30, 0x75, 0x18, 0xf7, 0xd8, 0x7a, 0x62, 0xe2, 0x89, 0x7a, 0x9f, 0x37, 0x32, 0xbb, 0xf6, 0x20,
];

const SEALED_INPUT_VERSION: &str = "issue407-stokes-dissipation-sealed-inputs-v1";
const E1_PROBE_IDENTITY: &str = "coordinate-a2";

/// The exact sealed `#407` input bytes.
pub(in crate::canonical_stokes) const fn e1_stokes_dissipation_sealed_inputs_v1() -> &'static [u8] {
    SEALED_INPUT_BYTES
}

/// One admitted sealed input, read only through the contract-frozen selectors.
pub(super) struct SealedE1Input {
    root: Json,
    design: [f64; 2],
    conjugate_design: [f64; 2],
}

impl SealedE1Input {
    /// Admit the exact sealed bytes and resolve the frozen E1 design selector.
    pub(super) fn admit(bytes: &[u8]) -> Result<Self, Diagnostic> {
        let digest: [u8; 32] = <sha2::Sha256 as sha2::Digest>::digest(bytes).into();
        if digest != SEALED_INPUT_SHA256 {
            return Err(invalid(
                "sealed E1 input bytes differ from the exact precommitted SHA-256 identity",
            ));
        }
        let root = Json::parse(bytes)?;
        if root.string("version")? != SEALED_INPUT_VERSION {
            return Err(invalid("sealed E1 input names another sealed version"));
        }
        let plan = root.get("derivative_observation_plan")?;
        let base = decimal_pair(plan.get("design")?)?;
        let probe = plan.get("probes")?.index(0)?;
        if probe.string("identity")? != E1_PROBE_IDENTITY {
            return Err(invalid("sealed E1 probe selector is not `coordinate-a2`"));
        }
        let direction = decimal_pair(probe.get("direction")?)?;
        let step = decimal(
            plan.get("finite_difference")?
                .get("step_sequence")?
                .index(0)?
                .as_string()?,
        )?;
        Ok(Self {
            design: scaled_design(base, direction, step),
            conjugate_design: scaled_design(base, direction, -step),
            root,
        })
    }

    /// Exact sealed plus-branch coefficients `a_E1_plus`.
    pub(super) const fn design_coefficients(&self) -> [f64; 2] {
        self.design
    }

    /// Exact sealed minus-branch coefficients, reserved for the design mutant.
    pub(super) const fn conjugate_design_coefficients(&self) -> [f64; 2] {
        self.conjugate_design
    }

    /// Verified identity of the admitted sealed bytes.
    pub(super) const fn sealed_input_sha256(&self) -> [u8; 32] {
        SEALED_INPUT_SHA256
    }

    /// Exact sealed mixed tolerance owning the analytic-area predicate.
    pub(super) fn analytic_area_tolerances(&self) -> Result<(f64, f64), Diagnostic> {
        let predicate = self
            .root
            .get("acceptance_predicates")?
            .get("analytic_area")?;
        Ok((
            dimensional_decimal(predicate.string("absolute_tolerance")?, "m^2")?,
            decimal(predicate.string("relative_tolerance")?)?,
        ))
    }

    /// Exact sealed coherent-SI equal-area radius.
    pub(super) fn area_radius_m(&self) -> Result<f64, Diagnostic> {
        self.dimensional_input("r_A", "m")
    }

    /// Exact sealed coherent-SI outer speed.
    pub(super) fn speed_m_per_s(&self) -> Result<f64, Diagnostic> {
        self.dimensional_input("U", "m/s")
    }

    /// Exact sealed coherent-SI dynamic viscosity.
    pub(super) fn dynamic_viscosity_pa_s(&self) -> Result<f64, Diagnostic> {
        self.dimensional_input("mu", "Pa*s")
    }

    fn dimensional_input(&self, name: &str, unit: &str) -> Result<f64, Diagnostic> {
        let entry = self.root.get("physical_inputs")?.get(name)?;
        if entry.string("unit")? != unit {
            return Err(invalid(
                "sealed physical input carries another coherent-SI unit",
            ));
        }
        decimal(entry.string("value")?)
    }

    /// Read one complete sealed topology member as an admissible source.
    pub(super) fn topology_source(
        &self,
        role: StokesDissipationTopologyRole2d,
    ) -> Result<StokesDissipationTopologySource2d, Diagnostic> {
        let (selector, expected_role) = match role {
            StokesDissipationTopologyRole2d::Reference => ("reference", "reference"),
            StokesDissipationTopologyRole2d::Refined => ("refined", "refined"),
        };
        let topology = self.root.get("topologies")?.get(selector)?;
        if topology.string("topology_role")? != expected_role {
            return Err(invalid("sealed topology member carries another exact role"));
        }
        self.require_sealed_mesh_obligations()?;
        let membership = topology.get("membership_counts")?;
        Ok(StokesDissipationTopologySource2d {
            role,
            content_identity: topology.string("content_identity")?.to_owned(),
            sector_count: topology.count("sector_count")?,
            radial_interval_count: topology.count("radial_interval_count")?,
            vertex_count: topology.count("vertex_count")?,
            cell_count: topology.count("cell_count")?,
            facet_count: topology.count("facet_count")?,
            membership_counts: [
                membership.count("body_boundary_vertices")?,
                membership.count("fluid_interior_vertices")?,
                membership.count("outer_boundary_vertices")?,
            ],
            vertices: topology
                .get("vertex_records")?
                .array()?
                .iter()
                .map(vertex_record)
                .collect::<Result<Vec<_>, _>>()?,
            cells: topology
                .get("cell_connectivity")?
                .array()?
                .iter()
                .map(cell_record)
                .collect::<Result<Vec<_>, _>>()?,
            boundary_facets: topology
                .get("boundary_facets")?
                .array()?
                .iter()
                .map(facet_record)
                .collect::<Result<Vec<_>, _>>()?,
            ordered_body_angles: topology
                .get("ordered_body_angle_samples")?
                .array()?
                .iter()
                .map(|value| value.as_string().map(str::to_owned))
                .collect::<Result<Vec<_>, _>>()?,
            correspondence: topology
                .get("correspondence")?
                .array()?
                .iter()
                .map(correspondence_record)
                .collect::<Result<Vec<_>, _>>()?,
            quality_gate: self.sealed_quality_gate()?,
            minimum_signed_area_m2: self.sealed_minimum_signed_area_m2()?,
            minimum_body_clearance_radius_multiple: self.sealed_clearance_radius_multiple()?,
            coordinate_tolerance_m: self.sealed_coordinate_tolerance_m()?,
        })
    }

    /// The mesh constructor's scale-invariant conditioning floor.
    ///
    /// Eqiora's mean-ratio conditioning is a distinct measure from the sealed
    /// scaled-Jacobian predicate, which stays owned by the sealed evidence and
    /// by the separate signed-area and clearance predicates. Only the sealed
    /// floor value is consumed here; no threshold is authored.
    fn sealed_quality_gate(&self) -> Result<MeshQualityGate, Diagnostic> {
        MeshQualityGate::new(decimal(
            self.mesh_quality()?.string("minimum_scaled_jacobian")?,
        )?)
    }

    fn sealed_minimum_signed_area_m2(&self) -> Result<f64, Diagnostic> {
        dimensional_decimal(self.mesh_quality()?.string("minimum_signed_area")?, "m^2")
    }

    fn sealed_clearance_radius_multiple(&self) -> Result<f64, Diagnostic> {
        let statement = self.mesh_quality()?.string("body_clearance_from_outer")?;
        let multiple = statement
            .split_once("> ")
            .and_then(|(_, tail)| tail.strip_suffix("*r_A"))
            .ok_or_else(|| invalid("sealed body-clearance predicate is not an `r_A` multiple"))?;
        decimal(multiple)
    }

    fn sealed_coordinate_tolerance_m(&self) -> Result<f64, Diagnostic> {
        dimensional_decimal(
            self.root
                .get("acceptance_predicates")?
                .get("analytic_profile_coordinate")?
                .string("absolute_tolerance")?,
            "m",
        )
    }

    /// The sealed mesh obligations this private realization depends on.
    ///
    /// The fixed-outer, simple-body, and exact-correspondence obligations are
    /// preconditions of the fixed-reference harmonic realization below, so a
    /// sealed input that withdrew any of them would invalidate it.
    fn require_sealed_mesh_obligations(&self) -> Result<(), Diagnostic> {
        let mesh_quality = self.mesh_quality()?;
        for obligation in [
            "outer_vertices_fixed",
            "body_polygon_simple",
            "harmonic_correspondence_exact",
        ] {
            if !mesh_quality.get(obligation)?.as_bool()? {
                return Err(invalid(
                    "sealed mesh predicate withdraws a required realization obligation",
                ));
            }
        }
        Ok(())
    }

    fn mesh_quality(&self) -> Result<&Json, Diagnostic> {
        self.root.get("acceptance_predicates")?.get("mesh_quality")
    }
}

fn scaled_design(base: [f64; 2], direction: [f64; 2], step: f64) -> [f64; 2] {
    [
        direction[0].mul_add(step, base[0]),
        direction[1].mul_add(step, base[1]),
    ]
}

fn vertex_record(value: &Json) -> Result<StokesDissipationVertexRecord2d, Diagnostic> {
    Ok(StokesDissipationVertexRecord2d {
        id: value.count("id")?,
        ring_index: value.count("ring_index")?,
        angle_index: value.count("angle_index")?,
        ring_fraction: value.string("ring_fraction")?.to_owned(),
        angle_turns: value.string("angle_turns")?.to_owned(),
        classification: value.string("classification")?.to_owned(),
    })
}

fn cell_record(value: &Json) -> Result<StokesDissipationCellRecord2d, Diagnostic> {
    let vertices = value.get("vertices")?.array()?;
    let [a, b, c] = vertices.as_slice() else {
        return Err(invalid("sealed cell record is not a triangle"));
    };
    Ok(StokesDissipationCellRecord2d {
        id: value.count("id")?,
        vertices: [a.as_count()?, b.as_count()?, c.as_count()?],
    })
}

fn facet_record(value: &Json) -> Result<StokesDissipationBoundaryFacetSource2d, Diagnostic> {
    let vertices = value.get("vertices")?.array()?;
    let [first, second] = vertices.as_slice() else {
        return Err(invalid("sealed facet record is not an edge"));
    };
    Ok(StokesDissipationBoundaryFacetSource2d {
        id: value.count("id")?,
        vertices: [first.as_count()?, second.as_count()?],
        kind: value.string("kind")?.to_owned(),
        label: value.string("label")?.to_owned(),
        orientation: value.string("orientation")?.to_owned(),
    })
}

fn correspondence_record(
    value: &Json,
) -> Result<StokesDissipationBodyCorrespondence2d, Diagnostic> {
    Ok(StokesDissipationBodyCorrespondence2d {
        angle_index: value.count("angle_index")?,
        angle_turns: value.string("angle_turns")?.to_owned(),
        body_vertex: value.count("body_vertex_id")?,
        body_facet: value.count("body_facet_id")?,
    })
}

fn decimal_pair(value: &Json) -> Result<[f64; 2], Diagnostic> {
    let members = value.array()?;
    let [first, second] = members.as_slice() else {
        return Err(invalid("sealed selector pair does not have two members"));
    };
    Ok([decimal(first.as_string()?)?, decimal(second.as_string()?)?])
}

fn dimensional_decimal(value: &str, unit: &str) -> Result<f64, Diagnostic> {
    let mut parts = value.split(' ');
    let magnitude = parts
        .next()
        .ok_or_else(|| invalid("sealed dimensional predicate is empty"))?;
    if parts.next() != Some(unit) || parts.next().is_some() {
        return Err(invalid(
            "sealed dimensional predicate carries another coherent-SI unit",
        ));
    }
    decimal(magnitude)
}

fn decimal(value: &str) -> Result<f64, Diagnostic> {
    let parsed = value
        .parse::<f64>()
        .map_err(|_| invalid("sealed real input is not a normalized decimal string"))?;
    if !parsed.is_finite() {
        return Err(invalid("sealed real input is not finite"));
    }
    Ok(parsed)
}

/// Minimal owned JSON value for reading the sealed input.
///
/// This is a private `cfg(test)` reader for one exact byte-identified input,
/// not a decoder, wire contract, or public artifact surface.
enum Json {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<Json>),
    Object(BTreeMap<String, Json>),
}

impl Json {
    fn parse(bytes: &[u8]) -> Result<Self, Diagnostic> {
        let text = std::str::from_utf8(bytes)
            .map_err(|_| invalid("sealed input is not valid UTF-8"))?
            .as_bytes();
        let mut reader = JsonReader { text, cursor: 0 };
        reader.skip_whitespace();
        let value = reader.value(0)?;
        reader.skip_whitespace();
        if reader.cursor != reader.text.len() {
            return Err(invalid("sealed input has trailing bytes after one value"));
        }
        Ok(value)
    }

    fn get(&self, key: &str) -> Result<&Self, Diagnostic> {
        match self {
            Self::Object(members) => members
                .get(key)
                .ok_or_else(|| invalid(format!("sealed input omits the member `{key}`"))),
            _ => Err(invalid(format!(
                "sealed selector `{key}` does not name an object"
            ))),
        }
    }

    fn index(&self, position: usize) -> Result<&Self, Diagnostic> {
        self.array()?
            .get(position)
            .ok_or_else(|| invalid("sealed selector index is out of range"))
    }

    fn array(&self) -> Result<&Vec<Self>, Diagnostic> {
        match self {
            Self::Array(members) => Ok(members),
            _ => Err(invalid("sealed selector does not name an array")),
        }
    }

    fn as_bool(&self) -> Result<bool, Diagnostic> {
        match self {
            Self::Bool(value) => Ok(*value),
            _ => Err(invalid("sealed selector does not name a boolean")),
        }
    }

    fn as_string(&self) -> Result<&str, Diagnostic> {
        match self {
            Self::String(value) => Ok(value),
            _ => Err(invalid("sealed selector does not name a string")),
        }
    }

    fn string(&self, key: &str) -> Result<&str, Diagnostic> {
        self.get(key)?.as_string()
    }

    fn as_count(&self) -> Result<usize, Diagnostic> {
        match self {
            Self::Number(value)
                if value.is_finite()
                    && *value >= 0.0
                    && value.fract() == 0.0
                    && *value < 1.0e18 =>
            {
                Ok(*value as usize)
            }
            _ => Err(invalid("sealed selector does not name a nonnegative index")),
        }
    }

    fn count(&self, key: &str) -> Result<usize, Diagnostic> {
        self.get(key)?.as_count()
    }
}

struct JsonReader<'a> {
    text: &'a [u8],
    cursor: usize,
}

impl JsonReader<'_> {
    fn value(&mut self, depth: usize) -> Result<Json, Diagnostic> {
        if depth > 32 {
            return Err(invalid("sealed input nests deeper than the accepted bound"));
        }
        match self.peek()? {
            b'{' => self.object(depth),
            b'[' => self.array(depth),
            b'"' => self.string().map(Json::String),
            b't' => self.literal("true").map(|()| Json::Bool(true)),
            b'f' => self.literal("false").map(|()| Json::Bool(false)),
            b'n' => self.literal("null").map(|()| Json::Null),
            _ => self.number(),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Json, Diagnostic> {
        self.expect(b'{')?;
        let mut members = BTreeMap::new();
        self.skip_whitespace();
        if self.peek()? == b'}' {
            self.cursor += 1;
            return Ok(Json::Object(members));
        }
        loop {
            self.skip_whitespace();
            let key = self.string()?;
            self.skip_whitespace();
            self.expect(b':')?;
            self.skip_whitespace();
            let value = self.value(depth + 1)?;
            if members.insert(key, value).is_some() {
                return Err(invalid("sealed input repeats one object key"));
            }
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.cursor += 1,
                b'}' => {
                    self.cursor += 1;
                    return Ok(Json::Object(members));
                }
                _ => return Err(invalid("sealed input object is malformed")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Json, Diagnostic> {
        self.expect(b'[')?;
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.peek()? == b']' {
            self.cursor += 1;
            return Ok(Json::Array(members));
        }
        loop {
            self.skip_whitespace();
            members.push(self.value(depth + 1)?);
            self.skip_whitespace();
            match self.peek()? {
                b',' => self.cursor += 1,
                b']' => {
                    self.cursor += 1;
                    return Ok(Json::Array(members));
                }
                _ => return Err(invalid("sealed input array is malformed")),
            }
        }
    }

    fn string(&mut self) -> Result<String, Diagnostic> {
        self.expect(b'"')?;
        let mut value = String::new();
        loop {
            let byte = self.next_byte()?;
            match byte {
                b'"' => return Ok(value),
                b'\\' => {
                    let escape = self.next_byte()?;
                    let decoded = match escape {
                        b'"' => '"',
                        b'\\' => '\\',
                        b'/' => '/',
                        b'b' => '\u{8}',
                        b'f' => '\u{c}',
                        b'n' => '\n',
                        b'r' => '\r',
                        b't' => '\t',
                        b'u' => self.unicode_escape()?,
                        _ => return Err(invalid("sealed input string has an unknown escape")),
                    };
                    value.push(decoded);
                }
                _ => {
                    let start = self.cursor - 1;
                    while self.peek().is_ok_and(|byte| byte != b'"' && byte != b'\\') {
                        self.cursor += 1;
                    }
                    value.push_str(
                        std::str::from_utf8(&self.text[start..self.cursor])
                            .map_err(|_| invalid("sealed input string is not valid UTF-8"))?,
                    );
                }
            }
        }
    }

    fn unicode_escape(&mut self) -> Result<char, Diagnostic> {
        if self.cursor + 4 > self.text.len() {
            return Err(invalid("sealed input string escape is truncated"));
        }
        let digits = std::str::from_utf8(&self.text[self.cursor..self.cursor + 4])
            .map_err(|_| invalid("sealed input string escape is not ASCII"))?;
        self.cursor += 4;
        u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| invalid("sealed input string escape is not one scalar value"))
    }

    fn number(&mut self) -> Result<Json, Diagnostic> {
        let start = self.cursor;
        while self
            .peek()
            .is_ok_and(|byte| matches!(byte, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9'))
        {
            self.cursor += 1;
        }
        std::str::from_utf8(&self.text[start..self.cursor])
            .ok()
            .and_then(|text| text.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .map(Json::Number)
            .ok_or_else(|| invalid("sealed input number is malformed"))
    }

    fn literal(&mut self, expected: &str) -> Result<(), Diagnostic> {
        if self.text[self.cursor..].starts_with(expected.as_bytes()) {
            self.cursor += expected.len();
            return Ok(());
        }
        Err(invalid("sealed input literal is malformed"))
    }

    fn skip_whitespace(&mut self) {
        while self
            .peek()
            .is_ok_and(|byte| matches!(byte, b' ' | b'\t' | b'\n' | b'\r'))
        {
            self.cursor += 1;
        }
    }

    fn peek(&self) -> Result<u8, Diagnostic> {
        self.text
            .get(self.cursor)
            .copied()
            .ok_or_else(|| invalid("sealed input ended before a complete value"))
    }

    fn next_byte(&mut self) -> Result<u8, Diagnostic> {
        let byte = self.peek()?;
        self.cursor += 1;
        Ok(byte)
    }

    fn expect(&mut self, byte: u8) -> Result<(), Diagnostic> {
        if self.next_byte()? == byte {
            return Ok(());
        }
        Err(invalid("sealed input is not canonical JSON"))
    }
}
