use eqiora_assembly::{AssemblyMap, DofId, LocalContribution, LocalUnknown, TargetAssemblyMap};

use crate::constrained_dofs::ConstrainedDofLayout;

use super::*;

#[test]
fn boundary_packets_follow_cells_and_preserve_constrained_flux_rows() {
    let mut fixture = Fixture::new(1, false);
    let constraints = ConstrainedDofLayout::new(fixture.fixed.clone()).unwrap();
    let packet = AssemblyPacket::new(
        LocalContribution::new(2, 2, vec![0.0; 4], vec![2.0, 3.0]).unwrap(),
        vec![
            TargetAssemblyMap::new(
                fixture.plan.target_id(0).unwrap(),
                constraints.reduced_map(&[0, 1]).unwrap(),
            ),
            TargetAssemblyMap::new(
                fixture.plan.target_id(1).unwrap(),
                constraints.full_map(&[0, 1]).unwrap(),
            ),
        ],
    )
    .unwrap();
    let base = fixture.prepare().unwrap();
    let base_result = REFERENCE_ASSEMBLY_BACKEND
        .assemble(&fixture.plan, &base)
        .unwrap();
    fixture.boundary_packets.push(packet.clone());
    let prepared = fixture.prepare().unwrap();
    assert_eq!(prepared.packet_count(), base.packet_count() + 1);
    assert_eq!(prepared.evaluate(base.packet_count()).unwrap(), packet);
    assert!(prepared.evaluate(prepared.packet_count()).is_err());
    let result = REFERENCE_ASSEMBLY_BACKEND
        .assemble(&fixture.plan, &prepared)
        .unwrap();
    let (base_systems, _) = base_result.into_parts();
    let (systems, _) = result.into_parts();
    assert_eq!(dense(&systems[0]), dense(&base_systems[0]));
    assert_eq!(dense(&systems[1]), dense(&base_systems[1]));
    close(systems[0].rhs()[0], base_systems[0].rhs()[0] + 3.0);
    close(systems[1].rhs()[0], base_systems[1].rhs()[0] + 2.0);
    close(systems[1].rhs()[1], base_systems[1].rhs()[1] + 3.0);
    close(systems[1].rhs()[2], base_systems[1].rhs()[2]);
}

#[test]
fn preparation_rejects_boundary_dofs_outside_the_target() {
    let mut fixture = Fixture::new(1, false);
    let outside = DofId::new(fixture.rhs.len());
    fixture.boundary_packets.push(
        AssemblyPacket::new(
            LocalContribution::new(1, 1, vec![0.0], vec![1.0]).unwrap(),
            vec![TargetAssemblyMap::new(
                fixture.plan.target_id(1).unwrap(),
                AssemblyMap::new(vec![Some(outside)], vec![LocalUnknown::Free(outside)]).unwrap(),
            )],
        )
        .unwrap(),
    );
    assert!(
        fixture
            .prepare()
            .unwrap_err()
            .message()
            .contains("global DOF outside its target")
    );
}
