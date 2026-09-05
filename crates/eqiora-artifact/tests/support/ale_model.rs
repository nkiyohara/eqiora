use eqiora_artifact::ModelEnvelope;
use eqiora_compiler::compile;
use eqiora_core::{Id, entity::kinds};
use eqiora_graph::{GraphStore, InMemoryGraphStore};
use eqiora_schema::kernel::KernelNode;
use eqiora_sem::KernelProgram;

#[derive(Clone, Copy)]
pub(crate) struct Ids {
    pub(crate) fluid_domain: Id<kinds::Domain>,
    pub(crate) solid_domain: Id<kinds::Domain>,
    pub(crate) fluid_velocity: Id<kinds::Field>,
    pub(crate) pressure: Id<kinds::Field>,
    pub(crate) solid_velocity: Id<kinds::Field>,
    pub(crate) displacement: Id<kinds::Field>,
    pub(crate) connection: Id<kinds::Connection>,
    pub(crate) fluid_relation: Id<kinds::Relation>,
    pub(crate) solid_relation: Id<kinds::Relation>,
}

impl Ids {
    pub(crate) fn model(dimension: usize) -> (ModelEnvelope, Self) {
        let mut boundaries = String::new();
        for body in ["fluid", "solid"] {
            for axis in 0..dimension {
                for side in ["lower", "upper"] {
                    if axis == 0
                        && ((body == "fluid" && side == "upper")
                            || (body == "solid" && side == "lower"))
                    {
                        continue;
                    }
                    boundaries.push_str(&format!("domain {body}_{axis}_{side} = boundary({body}, axis = {axis}, side = {side});\n"));
                }
            }
        }
        let source = r#"
public connector Mechanical = field_physical(
  trace = velocity: m / s, flux = traction: kg / (m * s ^ 2),
  shape = spatial_vector, frame = spatial, pairing = euclidean_boundary_duality
);
public component Side {
  public support body: volume(ambient_dimension = DIM);
  public support face: boundary(parent = body);
  public port mechanical: conserving Mechanical over face;
  relation retain continuous on face {
    trace(mechanical) = 0;
    flux(mechanical) = 0;
  }
}
model Main {
  domain fluid = box(BOX);
  domain solid = box(1, 2, REST);
  domain fluid_face = boundary(fluid, axis = 0, side = upper);
  domain solid_face = boundary(solid, axis = 0, side = lower);
  BOUNDARIES
  representation space = continuum;
  field fluid_velocity on fluid as space: m / s shape spatial_vector;
  field pressure on fluid as space: kg / (m * s ^ 2);
  field solid_velocity on solid as space: m / s shape spatial_vector;
  field displacement on solid as space: m shape spatial_vector;
  relation fluid_relation continuous on fluid { fluid_velocity = 0; pressure = 0; }
  relation solid_relation continuous on solid { solid_velocity = 0; displacement = 0; }
  instance left: Side(support body = fluid, support face = fluid_face);
  instance right: Side(support body = solid, support face = solid_face);
  connect conserving left.mechanical, right.mechanical;
}
"#
        .replace("BOUNDARIES", &boundaries)
        .replace("DIM", &dimension.to_string())
        .replace("BOX", &vec!["0, 1"; dimension].join(", "))
        .replace("REST", &vec!["0, 1"; dimension - 1].join(", "));
        let compiled = compile("ale-wire.eqi", &source).unwrap().remove(0);
        let symbols = compiled.symbols().clone();
        let (transaction, model_id, _) = compiled.into_parts();
        let mut store = InMemoryGraphStore::new();
        store.commit(transaction).unwrap();
        let program = KernelProgram::from_snapshot(&store.snapshot(), model_id).unwrap();
        let connection = program
            .nodes()
            .find_map(|node| match node {
                KernelNode::Connection(connection) => Some(connection.id()),
                _ => None,
            })
            .unwrap();
        let ids = Self {
            fluid_domain: symbols.get("fluid").unwrap().downcast().unwrap(),
            solid_domain: symbols.get("solid").unwrap().downcast().unwrap(),
            fluid_velocity: symbols.get("fluid_velocity").unwrap().downcast().unwrap(),
            pressure: symbols.get("pressure").unwrap().downcast().unwrap(),
            solid_velocity: symbols.get("solid_velocity").unwrap().downcast().unwrap(),
            displacement: symbols.get("displacement").unwrap().downcast().unwrap(),
            connection,
            fluid_relation: symbols.get("fluid_relation").unwrap().downcast().unwrap(),
            solid_relation: symbols.get("solid_relation").unwrap().downcast().unwrap(),
        };
        (ModelEnvelope::from_program(&program).unwrap(), ids)
    }
}
