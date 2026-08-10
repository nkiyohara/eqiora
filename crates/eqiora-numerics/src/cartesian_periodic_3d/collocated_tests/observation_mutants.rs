use std::collections::BTreeSet;

use super::{
    CELLS, Fixture, PREDECESSOR_MESH_SHA256, ValidationFault, ViewObservation, parse_hex_digest,
    validate_view,
};

#[derive(Debug, Clone, Copy)]
enum ObservationMutant {
    PacketDrop,
    SeamFlag,
    TangentialShift,
    SeamDouble,
    AreaSwap,
    DistanceUlp,
    NormalSign,
    FaceFamily,
    ConnectionOrder,
    MeshIdentity,
    ModelRevision,
    ParentConnectorSwap,
    PacketIdRenumber,
}

impl ObservationMutant {
    const ALL: [Self; 13] = [
        Self::PacketDrop,
        Self::SeamFlag,
        Self::TangentialShift,
        Self::SeamDouble,
        Self::AreaSwap,
        Self::DistanceUlp,
        Self::NormalSign,
        Self::FaceFamily,
        Self::ConnectionOrder,
        Self::MeshIdentity,
        Self::ModelRevision,
        Self::ParentConnectorSwap,
        Self::PacketIdRenumber,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::PacketDrop => "CV4-OBS-PACKET-DROP",
            Self::SeamFlag => "CV4-OBS-SEAM-FLAG",
            Self::TangentialShift => "CV4-OBS-TANGENTIAL-SHIFT",
            Self::SeamDouble => "CV4-OBS-SEAM-DOUBLE",
            Self::AreaSwap => "CV4-OBS-AREA-SWAP",
            Self::DistanceUlp => "CV4-OBS-DISTANCE-ULP",
            Self::NormalSign => "CV4-OBS-NORMAL-SIGN",
            Self::FaceFamily => "CV4-OBS-FACE-FAMILY",
            Self::ConnectionOrder => "CV4-OBS-CONNECTION-ORDER",
            Self::MeshIdentity => "CV4-OBS-MESH-IDENTITY",
            Self::ModelRevision => "CV4-OBS-MODEL-REVISION",
            Self::ParentConnectorSwap => "CV4-OBS-PARENT-CONNECTOR-SWAP",
            Self::PacketIdRenumber => "CV4-OBS-PACKET-ID-RENUMBER",
        }
    }

    const fn expected_fault(self) -> ValidationFault {
        match self {
            Self::PacketDrop => ValidationFault::PacketCount,
            Self::SeamFlag => ValidationFault::SeamLaw,
            Self::TangentialShift => ValidationFault::NeighborLaw,
            Self::SeamDouble => ValidationFault::ReverseDuplicate,
            Self::AreaSwap => ValidationFault::FaceAreaPartition,
            Self::DistanceUlp => ValidationFault::LiftedDistanceBits,
            Self::NormalSign => ValidationFault::NormalLaw,
            Self::FaceFamily => ValidationFault::QuotientFaceLaw,
            Self::ConnectionOrder => ValidationFault::ConnectionAxisOrder,
            Self::MeshIdentity => ValidationFault::MeshIdentity,
            Self::ModelRevision => ValidationFault::ModelIdentity,
            Self::ParentConnectorSwap => ValidationFault::ParentConnectorIdentity,
            Self::PacketIdRenumber => ValidationFault::PacketIdentity,
        }
    }
}

pub(super) fn assert_observation_mutants(fixture: &Fixture, positive: &ViewObservation) {
    let mut exercised = BTreeSet::new();
    for mutant in ObservationMutant::ALL {
        let mut observation = positive.clone();
        apply_observation_mutant(mutant, &mut observation);
        assert_ne!(
            observation,
            *positive,
            "{} did not alter the accepted observation",
            mutant.name()
        );
        assert_eq!(
            validate_view(fixture, &observation),
            Err(mutant.expected_fault()),
            "{} must fail at its named boundary",
            mutant.name()
        );
        assert!(exercised.insert(mutant.name()));
    }
    assert_eq!(exercised.len(), ObservationMutant::ALL.len());
}

fn apply_observation_mutant(mutant: ObservationMutant, observation: &mut ViewObservation) {
    match mutant {
        ObservationMutant::PacketDrop => {
            observation.packets.pop().expect("positive packet");
        }
        ObservationMutant::SeamFlag => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| !packet.seam)
                .expect("interior packet");
            packet.seam = true;
        }
        ObservationMutant::TangentialShift => {
            let packet = observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 0 && packet.seam)
                .expect("axis-0 seam packet");
            packet.neighbor_cell += 8;
        }
        ObservationMutant::SeamDouble => {
            let mut packet = observation
                .packets
                .iter()
                .find(|packet| packet.seam)
                .expect("seam packet")
                .clone();
            std::mem::swap(&mut packet.owner_cell, &mut packet.neighbor_cell);
            packet.normal = packet.normal.map(|value| -value);
            observation.packets.push(packet);
        }
        ObservationMutant::AreaSwap => {
            let axis_0 = observation
                .packets
                .iter()
                .position(|packet| packet.axis == 0)
                .expect("axis-0 packet");
            let axis_2 = observation
                .packets
                .iter()
                .position(|packet| packet.axis == 2)
                .expect("axis-2 packet");
            let saved = observation.packets[axis_0].face_area_bits;
            observation.packets[axis_0].face_area_bits = observation.packets[axis_2].face_area_bits;
            observation.packets[axis_2].face_area_bits = saved;
        }
        ObservationMutant::DistanceUlp => {
            observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 1)
                .expect("axis-1 packet")
                .lifted_center_distance_bits = 0x3FF2_AAAA_AAAA_AAAC;
        }
        ObservationMutant::NormalSign => {
            let packet = observation.packets.first_mut().expect("positive packet");
            packet.normal = packet.normal.map(|value| -value);
        }
        ObservationMutant::FaceFamily => {
            observation
                .packets
                .iter_mut()
                .find(|packet| packet.axis == 2)
                .expect("axis-2 packet")
                .quotient_face += CELLS;
        }
        ObservationMutant::ConnectionOrder => observation.connections.swap(0, 2),
        ObservationMutant::MeshIdentity => {
            observation.mesh_artifact_sha256 = parse_hex_digest(PREDECESSOR_MESH_SHA256);
        }
        ObservationMutant::ModelRevision => observation.semantic_revision += 1,
        ObservationMutant::ParentConnectorSwap => {
            std::mem::swap(&mut observation.parent, &mut observation.connector);
        }
        ObservationMutant::PacketIdRenumber => {
            let packet = observation.packets.first_mut().expect("positive packet");
            packet.packet = packet.axis * CELLS + packet.neighbor_cell;
        }
    }
}
