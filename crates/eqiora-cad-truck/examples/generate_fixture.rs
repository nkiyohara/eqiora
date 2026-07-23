use truck_modeling::{Point3, Vector3, builder};
use truck_stepio::out::{CompleteStepDisplay, StepHeaderDescriptor, StepModel};

fn main() {
    let vertex = builder::vertex(Point3::new(-1000.0, -1000.0, -1000.0));
    let edge = builder::tsweep(&vertex, Vector3::new(2000.0, 0.0, 0.0));
    let face = builder::tsweep(&edge, Vector3::new(0.0, 2000.0, 0.0));
    let solid = builder::tsweep(&face, Vector3::new(0.0, 0.0, 2000.0));
    let compressed = solid.compress();
    print!(
        "{}",
        CompleteStepDisplay::new(
            StepModel::from(&compressed),
            StepHeaderDescriptor {
                file_name: "outer-box-mm.step".to_owned(),
                time_stamp: "2000-01-01T00:00:00Z".to_owned(),
                organization_system: "eqiora-truck-fixture".to_owned(),
                ..Default::default()
            },
        )
    );
}
