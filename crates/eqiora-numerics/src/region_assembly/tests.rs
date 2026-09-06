use eqiora_assembly::{AssemblyBackend, LinearSystem, REFERENCE_ASSEMBLY_BACKEND};

use super::*;

mod fixture;
mod validation;
use fixture::Fixture;

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-11 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

fn dense(system: &LinearSystem) -> Vec<f64> {
    let matrix = system.matrix();
    let n = matrix.rows();
    let mut dense = vec![0.0; n * n];
    for row in 0..n {
        for slot in matrix.row_offsets()[row]..matrix.row_offsets()[row + 1] {
            dense[row * n + matrix.column_indices()[slot]] = matrix.values()[slot];
        }
    }
    dense
}

#[test]
fn two_and_three_regions_assemble_variable_fields_through_existing_constraint_maps() {
    for regions in [2, 3] {
        for reversed in [false, true] {
            let fixture = Fixture::new(regions, reversed);
            let prepared = fixture.prepare().unwrap();
            assert_eq!(prepared.packet_count(), 2 * regions);
            assert_eq!(prepared.packet_set_identity(), fixture.packet_set);
            assert_eq!(prepared, prepared.clone());
            let result = REFERENCE_ASSEMBLY_BACKEND
                .assemble(&fixture.plan, &prepared)
                .unwrap();
            let full = result.system(fixture.plan.target_id(1).unwrap()).unwrap();
            for (actual, expected) in dense(full).into_iter().zip(&fixture.matrix) {
                close(actual, *expected);
            }
            for (actual, expected) in full.rhs().iter().zip(&fixture.rhs) {
                close(*actual, *expected);
            }
            let reduced = result.system(fixture.plan.target_id(0).unwrap()).unwrap();
            let free = fixture
                .fixed
                .iter()
                .enumerate()
                .filter_map(|(i, value)| value.is_none().then_some(i))
                .collect::<Vec<_>>();
            let actual = dense(reduced);
            let n = fixture.rhs.len();
            for (row, &global_row) in free.iter().enumerate() {
                let mut rhs = fixture.rhs[global_row];
                for (column, fixed) in fixture.fixed.iter().enumerate() {
                    if let Some(value) = fixed {
                        rhs -= fixture.matrix[global_row * n + column] * value;
                    }
                }
                close(reduced.rhs()[row], rhs);
                for (column, &global_column) in free.iter().enumerate() {
                    close(
                        actual[row * free.len() + column],
                        fixture.matrix[global_row * n + global_column],
                    );
                }
            }
            // Retained packet contributions and projections are the same recovery input.
            for index in 0..prepared.packet_count() {
                let packet = prepared.evaluate(index).unwrap();
                assert_eq!(packet.mappings().len(), 2);
                assert_eq!(packet, prepared.clone().evaluate(index).unwrap());
                packet.project(&fixture.plan).unwrap();
            }
            assert!(
                prepared
                    .evaluate(prepared.packet_count())
                    .unwrap_err()
                    .message()
                    .contains("outside the prepared mesh")
            );
        }
    }
}
