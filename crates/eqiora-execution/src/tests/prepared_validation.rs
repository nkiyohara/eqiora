use super::*;

struct ChangedCoefficients([f64; 2]);

impl CompleteCsrStorage for ChangedCoefficients {
    fn rows(&self) -> usize {
        2
    }
    fn columns(&self) -> usize {
        2
    }
    fn row_offsets(&self) -> &[usize] {
        &[0, 2, 4]
    }
    fn column_indices(&self) -> &[usize] {
        &[0, 1, 0, 1]
    }
    fn values(&self) -> &[f64] {
        &[3.0, -1.0, -1.0, 3.0]
    }
    fn right_hand_side(&self) -> &[f64] {
        &self.0
    }
}

#[test]
fn commit_tracks_rhs_and_coefficients_after_acceptance_only() {
    let graph = portable_graph();
    let first_system = system([1.0, 0.0]);
    let first_binding = serial_binding(&graph);
    let plan = first_binding.solver_plan();
    let first = AdmittedExecution::admit_host_linear(&graph, &first_system, first_binding).unwrap();
    let mut prepared = PreparedLinearExecution::new();
    prepared
        .execute(first, |_, _, coefficients_reusable, accept| {
            assert!(!coefficients_reusable);
            accept(solve(&first_system, plan))
        })
        .unwrap();

    let changed_rhs = TwoByTwo {
        right_hand_side: [0.0, 1.0],
    };
    let changed_rhs = CanonicalCsrSystemView::new(
        &changed_rhs,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    let omitted =
        AdmittedExecution::admit_host_linear(&graph, &changed_rhs, serial_binding(&graph)).unwrap();
    let error = prepared
        .execute(omitted, |_, _, coefficients_reusable, _| {
            assert!(coefficients_reusable);
            Ok(())
        })
        .unwrap_err();
    assert!(error.message().contains("omitted execution acceptance"));

    let changed_coefficients = CanonicalCsrSystemView::new(
        &ChangedCoefficients([1.0, 0.0]),
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    let double =
        AdmittedExecution::admit_host_linear(&graph, &changed_coefficients, serial_binding(&graph))
            .unwrap();
    let error = prepared
        .execute(double, |_, _, coefficients_reusable, accept| {
            assert!(!coefficients_reusable);
            accept(solve(&changed_coefficients, plan))?;
            accept(solve(&changed_coefficients, plan))
        })
        .unwrap_err();
    assert!(error.message().contains("accepted more than once"));

    let candidate =
        AdmittedExecution::admit_host_linear(&graph, &changed_coefficients, serial_binding(&graph))
            .unwrap();
    let error = prepared
        .execute(candidate, |_, _, coefficients_reusable, _| {
            assert!(!coefficients_reusable);
            Err(Diagnostic::error(codes::INVALID_REALIZATION, "probe"))
        })
        .unwrap_err();
    assert_eq!(error.message(), "probe");

    let candidate =
        AdmittedExecution::admit_host_linear(&graph, &changed_rhs, serial_binding(&graph)).unwrap();
    let rejected = prepared
        .execute(candidate, |_, _, coefficients_reusable, accept| {
            assert!(coefficients_reusable);
            accept(solve(&first_system, plan))
        })
        .unwrap_err();
    assert_eq!(rejected.code(), codes::INVALID_REALIZATION);
    let retry =
        AdmittedExecution::admit_host_linear(&graph, &changed_rhs, serial_binding(&graph)).unwrap();
    let error = prepared
        .execute(retry, |_, _, coefficients_reusable, _| {
            assert!(
                coefficients_reusable,
                "rejected candidate replaced accepted commit"
            );
            Err(Diagnostic::error(codes::INVALID_REALIZATION, "probe"))
        })
        .unwrap_err();
    assert_eq!(error.message(), "probe");

    let mut separate = PreparedLinearExecution::new();
    let foreign =
        AdmittedExecution::admit_host_linear(&graph, &changed_rhs, serial_binding(&graph)).unwrap();
    let error = separate
        .execute(foreign, |_, _, coefficients_reusable, _| {
            assert!(
                !coefficients_reusable,
                "preparation crossed Run-local owners"
            );
            Err(Diagnostic::error(codes::INVALID_REALIZATION, "probe"))
        })
        .unwrap_err();
    assert_eq!(error.message(), "probe");
}

struct ChangedStructure;

impl CompleteCsrStorage for ChangedStructure {
    fn rows(&self) -> usize {
        2
    }
    fn columns(&self) -> usize {
        2
    }
    fn row_offsets(&self) -> &[usize] {
        &[0, 1, 2]
    }
    fn column_indices(&self) -> &[usize] {
        &[0, 1]
    }
    fn values(&self) -> &[f64] {
        &[2.0, 2.0]
    }
    fn right_hand_side(&self) -> &[f64] {
        &[1.0, 0.0]
    }
}

#[test]
fn rejects_foreign_binding_and_structure_before_action() {
    let graph = portable_graph();
    let original = system([1.0, 0.0]);
    let plan = serial_binding(&graph).solver_plan();
    let admitted =
        AdmittedExecution::admit_host_linear(&graph, &original, serial_binding(&graph)).unwrap();
    let mut prepared = PreparedLinearExecution::new();
    prepared
        .execute(admitted, |_, _, _, accept| accept(solve(&original, plan)))
        .unwrap();

    let changed = CanonicalCsrSystemView::new(
        &ChangedStructure,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )
    .unwrap();
    let admitted =
        AdmittedExecution::admit_host_linear(&graph, &changed, serial_binding(&graph)).unwrap();
    let error = prepared
        .execute(admitted, |_, _, _, _| {
            panic!("structure drift reached provider")
        })
        .unwrap_err();
    assert_eq!(error.code(), codes::INVALID_REALIZATION);
    assert!(error.message().contains("sparse structure"));

    let foreign_graph = portable_graph();
    let admitted = AdmittedExecution::admit_host_linear(
        &foreign_graph,
        &original,
        serial_binding(&foreign_graph),
    )
    .unwrap();
    let error = prepared
        .execute(admitted, |_, _, _, _| {
            panic!("foreign graph reached provider")
        })
        .unwrap_err();
    assert_eq!(error.code(), codes::INVALID_REALIZATION);
    assert!(error.message().contains("portable graph"));
}
