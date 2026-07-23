//! Bounded Truck adapter for the first Eqiora CAD vertical slice.
//!
//! Truck owns STEP parsing and B-rep/modeling objects only inside this crate.
//! The admitted source is one closed, line/plane, axis-aligned cuboid. The
//! constrained rectangle extrusion is built with Truck modeling primitives;
//! the single boolean is the exact intersection of two axis-aligned boxes and
//! is reconstructed as a fresh six-plane Truck B-rep. `truck-shapeops` is
//! deliberately not a dependency because its current graph fails the security
//! admission gate.

use std::collections::BTreeSet;

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use eqiora_geometry::{
    AxisAlignedBox3, CadAdapterIdentityV1, CadBoxDesignV1, CadBoxObservationV1,
    CadBoxRealizationV1, CadKernelAdapter, CadRepairDispositionV1, StepLengthUnitV1,
    StepSourceDigest,
};
use truck_modeling::{Point3, Solid, Vector3, builder};
use truck_stepio::r#in::alias::{Curve3D, ElementarySurface, Surface as StepSurface};
use truck_stepio::r#in::ruststep::ast::{EntityInstance, Parameter, Record};
use truck_stepio::r#in::{Table, ruststep};
use truck_topology::compress::CompressedShell;

/// Stable adapter identity recorded in CAD build evidence.
pub const ADAPTER_ID: &str = "eqiora.cad.truck-box-v1";
/// Exact upstream implementation identity admitted by this build.
pub const KERNEL_ID: &str = "truck-stepio-modeling-topology";
/// Exact upstream crate versions admitted by this build.
pub const KERNEL_VERSION: &str = "stepio-0.3.0+modeling-0.6.0+topology-0.6.0";
/// Maximum complete STEP bytes admitted before UTF-8 or parser work.
pub const MAX_STEP_SOURCE_BYTES: usize = 1024 * 1024;
/// Maximum entity instances admitted before Table construction.
pub const MAX_STEP_ENTITIES: usize = 20_000;

/// Stateless, resource-bounded Truck adapter.
#[derive(Clone, Copy, Debug, Default)]
pub struct TruckCadAdapterV1;

impl CadKernelAdapter for TruckCadAdapterV1 {
    fn identity(&self) -> CadAdapterIdentityV1 {
        CadAdapterIdentityV1::new(
            ADAPTER_ID,
            env!("CARGO_PKG_VERSION"),
            KERNEL_ID,
            KERNEL_VERSION,
        )
    }

    fn realize_box_design(
        &self,
        design: &CadBoxDesignV1,
        step_bytes: &[u8],
    ) -> Result<CadBoxRealizationV1, Diagnostic> {
        if step_bytes.len() > MAX_STEP_SOURCE_BYTES {
            return Err(invalid_cad("STEP source exceeds the adapter byte limit"));
        }
        if StepSourceDigest::from_source_bytes(step_bytes) != design.source() {
            return Err(invalid_cad(
                "complete STEP bytes differ from the exact design source digest",
            ));
        }
        let step = std::str::from_utf8(step_bytes)
            .map_err(|_| invalid_cad("STEP source must be UTF-8 ASCII-compatible text"))?;
        let exchange = ruststep::parser::parse(step)
            .map_err(|error| invalid_cad(format!("invalid STEP exchange structure: {error}")))?;
        if exchange.data.len() != 1 || exchange.data[0].entities.len() > MAX_STEP_ENTITIES {
            return Err(invalid_cad("CAD v1 requires one bounded STEP DATA section"));
        }
        validate_source_structure(&exchange.data[0].entities, design.source_length_unit())?;

        let mut table = Table::default();
        for entity in &exchange.data[0].entities {
            table
                .push_instance(entity)
                .map_err(|error| invalid_cad(format!("unsupported STEP entity: {error}")))?;
        }
        if table.shell.len() != 1 {
            return Err(invalid_cad(
                "CAD v1 requires exactly one imported STEP shell",
            ));
        }
        let shell = table
            .to_compressed_shell(table.shell.values().next().expect("one checked shell"))
            .map_err(|error| invalid_cad(format!("cannot construct STEP B-rep: {error}")))?;
        validate_imported_box(&shell, design)?;

        let imported_stock = accepted_observation(design.imported_stock())?;
        let tool_bounds = extruded_tool_bounds(design)?;
        let tool = truck_box(tool_bounds);
        validate_truck_box(&tool, tool_bounds, design.modeling_tolerance_m())?;
        let extruded_tool = accepted_observation(tool_bounds)?;

        let intersection_bounds = design.imported_stock().intersection(tool_bounds)?;
        if intersection_bounds != design.output() {
            return Err(invalid_cad(
                "adapter boolean differs from the exact design intersection",
            ));
        }
        let intersection = truck_box(intersection_bounds);
        validate_truck_box(
            &intersection,
            intersection_bounds,
            design.modeling_tolerance_m(),
        )?;
        let intersection = accepted_observation(intersection_bounds)?;

        CadBoxRealizationV1::new(design, imported_stock, extruded_tool, intersection)
    }
}

fn validate_source_structure(
    entities: &[EntityInstance],
    expected_unit: StepLengthUnitV1,
) -> Result<(), Diagnostic> {
    let records = entities.iter().flat_map(records);
    let names = records.clone().map(|record| record.name.as_str());
    let closed_shells = names.clone().filter(|name| *name == "CLOSED_SHELL").count();
    let open_shells = names.clone().filter(|name| *name == "OPEN_SHELL").count();
    let solids = names
        .clone()
        .filter(|name| *name == "MANIFOLD_SOLID_BREP")
        .count();
    if closed_shells != 1 || open_shells != 0 || solids != 1 {
        return Err(invalid_cad(
            "CAD v1 requires one MANIFOLD_SOLID_BREP over one CLOSED_SHELL and no OPEN_SHELL",
        ));
    }

    let unit_records = entities
        .iter()
        .filter_map(|entity| match entity {
            EntityInstance::Complex { subsuper, .. }
                if subsuper.0.iter().any(|record| record.name == "LENGTH_UNIT")
                    && subsuper.0.iter().any(|record| record.name == "NAMED_UNIT") =>
            {
                subsuper.0.iter().find(|record| record.name == "SI_UNIT")
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if unit_records.len() != 1 || decode_length_unit(unit_records[0]) != Some(expected_unit) {
        return Err(invalid_cad(
            "STEP length unit differs from the exact metre/millimetre design contract",
        ));
    }
    Ok(())
}

fn records(entity: &EntityInstance) -> impl Clone + Iterator<Item = &Record> {
    let slice = match entity {
        EntityInstance::Simple { record, .. } => std::slice::from_ref(record),
        EntityInstance::Complex { subsuper, .. } => subsuper.0.as_slice(),
    };
    slice.iter()
}

fn decode_length_unit(record: &Record) -> Option<StepLengthUnitV1> {
    let Parameter::List(parameters) = &record.parameter else {
        return None;
    };
    match parameters.as_slice() {
        [Parameter::NotProvided, Parameter::Enumeration(unit)] if unit == "METRE" => {
            Some(StepLengthUnitV1::Metre)
        }
        [Parameter::Enumeration(prefix), Parameter::Enumeration(unit)]
            if prefix == "MILLI" && unit == "METRE" =>
        {
            Some(StepLengthUnitV1::Millimetre)
        }
        _ => None,
    }
}

fn validate_imported_box(
    shell: &CompressedShell<Point3, Curve3D, StepSurface>,
    design: &CadBoxDesignV1,
) -> Result<(), Diagnostic> {
    if shell.vertices.len() != 8 || shell.edges.len() != 12 || shell.faces.len() != 6 {
        return Err(invalid_cad(
            "STEP stock must contain exactly 8 vertices, 12 edges, and 6 faces",
        ));
    }
    if shell
        .edges
        .iter()
        .any(|edge| !matches!(edge.curve, Curve3D::Line(_)))
        || shell.faces.iter().any(|face| {
            !matches!(
                face.surface,
                StepSurface::ElementarySurface(ref surface)
                    if matches!(surface.as_ref(), ElementarySurface::Plane(_))
            )
        })
    {
        return Err(invalid_cad(
            "STEP stock must use only straight edges and planar faces",
        ));
    }

    let scale = design.source_length_unit().metres_per_source_unit();
    let bounds = design.imported_stock().bounds_m();
    let tolerance = design.source_uncertainty_m();
    let mut corners = BTreeSet::new();
    for point in &shell.vertices {
        let coordinates = [point.x * scale, point.y * scale, point.z * scale];
        let mut corner = 0_u8;
        for axis in 0..3 {
            if near(coordinates[axis], bounds[axis].0, tolerance) {
                continue;
            }
            if near(coordinates[axis], bounds[axis].1, tolerance) {
                corner |= 1 << axis;
                continue;
            }
            return Err(invalid_cad(
                "STEP vertex is not on an expected stock-box corner",
            ));
        }
        if !corners.insert(corner) {
            return Err(invalid_cad("STEP stock repeats a box corner"));
        }
    }
    if corners.len() != 8 {
        return Err(invalid_cad("STEP stock does not contain all box corners"));
    }

    let mut edge_use = vec![0_usize; shell.edges.len()];
    let mut roles = BTreeSet::new();
    for face in &shell.faces {
        if face.boundaries.len() != 1 || face.boundaries[0].len() != 4 {
            return Err(invalid_cad(
                "STEP box face must have one four-edge outer boundary and no holes",
            ));
        }
        let mut face_vertices = BTreeSet::new();
        for edge_ref in &face.boundaries[0] {
            let Some(edge) = shell.edges.get(edge_ref.index) else {
                return Err(invalid_cad("STEP face references an unknown edge"));
            };
            edge_use[edge_ref.index] += 1;
            if edge.vertices.0 >= shell.vertices.len()
                || edge.vertices.1 >= shell.vertices.len()
                || edge.vertices.0 == edge.vertices.1
            {
                return Err(invalid_cad("STEP edge has invalid endpoint indices"));
            }
            face_vertices.insert(edge.vertices.0);
            face_vertices.insert(edge.vertices.1);
        }
        if face_vertices.len() != 4 {
            return Err(invalid_cad("STEP box face must contain four corners"));
        }
        let role = (0..3).find_map(|axis| {
            [bounds[axis].0, bounds[axis].1]
                .into_iter()
                .enumerate()
                .find_map(|(side, coordinate)| {
                    face_vertices
                        .iter()
                        .all(|&vertex| {
                            let point = &shell.vertices[vertex];
                            near(
                                [point.x, point.y, point.z][axis] * scale,
                                coordinate,
                                tolerance,
                            )
                        })
                        .then_some((axis, side))
                })
        });
        let Some(role) = role else {
            return Err(invalid_cad(
                "STEP planar face is not one complete axis-aligned stock side",
            ));
        };
        if !roles.insert(role) {
            return Err(invalid_cad("STEP stock contains a split or repeated side"));
        }
    }
    if roles.len() != 6 || edge_use.iter().any(|&uses| uses != 2) {
        return Err(invalid_cad(
            "STEP stock is not one closed six-side manifold shell",
        ));
    }
    Ok(())
}

fn extruded_tool_bounds(design: &CadBoxDesignV1) -> Result<AxisAlignedBox3, Diagnostic> {
    let sketch = design.sketch();
    AxisAlignedBox3::new([
        sketch.x_bounds_m(),
        sketch.y_bounds_m(),
        (
            sketch.plane_z_m(),
            sketch.plane_z_m() + design.extrusion_depth_m(),
        ),
    ])
}

fn truck_box(bounds: AxisAlignedBox3) -> Solid {
    let [x, y, z] = bounds.bounds_m();
    let vertex = builder::vertex(Point3::new(x.0, y.0, z.0));
    let edge = builder::tsweep(&vertex, Vector3::new(x.1 - x.0, 0.0, 0.0));
    let face = builder::tsweep(&edge, Vector3::new(0.0, y.1 - y.0, 0.0));
    builder::tsweep(&face, Vector3::new(0.0, 0.0, z.1 - z.0))
}

fn validate_truck_box(
    solid: &Solid,
    expected: AxisAlignedBox3,
    tolerance: f64,
) -> Result<(), Diagnostic> {
    let compressed = solid.compress();
    if compressed.boundaries.len() != 1 {
        return Err(invalid_cad(
            "Truck modeling result must contain one closed shell",
        ));
    }
    let shell = &compressed.boundaries[0];
    if shell.vertices.len() != 8 || shell.edges.len() != 12 || shell.faces.len() != 6 {
        return Err(invalid_cad(
            "Truck modeling result must be one six-plane cuboid",
        ));
    }
    let bounds = expected.bounds_m();
    for point in &shell.vertices {
        for (axis, value) in [point.x, point.y, point.z].into_iter().enumerate() {
            if !near(value, bounds[axis].0, tolerance) && !near(value, bounds[axis].1, tolerance) {
                return Err(invalid_cad(
                    "Truck modeling vertex differs from the exact box result",
                ));
            }
        }
    }
    Ok(())
}

fn accepted_observation(bounds: AxisAlignedBox3) -> Result<CadBoxObservationV1, Diagnostic> {
    CadBoxObservationV1::new(bounds, 1, 1, 6, CadRepairDispositionV1::None)
}

fn near(actual: f64, expected: f64, tolerance: f64) -> bool {
    actual.is_finite() && (actual - expected).abs() <= tolerance
}

fn invalid_cad(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_ARTIFACT, message)
}
