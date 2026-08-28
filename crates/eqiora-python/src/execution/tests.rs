use std::sync::Arc;
use std::thread;

use eqiora::Diagnostic;
use eqiora::diagnostic::codes;

use super::{
    CacheClaim, MaterializationClaim, PyRunStatus, ResultCache, RunFailure, RunShared, RunState,
    RunTerminal, RunTerminalKind,
};

#[test]
fn run_states_follow_the_explicit_branching_transition_table() {
    let mut completed = RunState::accepted();
    completed.transition(PyRunStatus::Running);
    completed.transition(PyRunStatus::Cancelling);
    completed.transition(PyRunStatus::Completed);
    assert_eq!(
        completed.history,
        [
            PyRunStatus::Created,
            PyRunStatus::Validating,
            PyRunStatus::Queued,
            PyRunStatus::Running,
            PyRunStatus::Cancelling,
            PyRunStatus::Completed,
        ]
    );
    assert!(!completed.integrity_failed);

    completed.transition(PyRunStatus::Cancelled);
    assert!(completed.integrity_failed);
    assert_eq!(completed.status, PyRunStatus::Failed);
    assert!(matches!(
        completed.terminal,
        Some(RunTerminal::Failed(RunFailure::Internal(_)))
    ));

    let mut cancelled = RunState::accepted();
    cancelled.transition(PyRunStatus::Cancelling);
    cancelled.transition(PyRunStatus::Cancelled);
    assert_eq!(cancelled.history.len(), 5);
    assert!(!cancelled.integrity_failed);
}

#[test]
fn every_waiter_observes_one_failed_terminal() {
    let shared = Arc::new(RunShared::new());
    let waiters: Vec<_> = (0..2)
        .map(|_| {
            let shared = Arc::clone(&shared);
            thread::spawn(move || shared.wait_terminal_kind())
        })
        .collect();
    shared.finish(RunTerminal::Failed(RunFailure::Execution(vec![
        Diagnostic::error(codes::NONSQUARE_SYSTEM, "probe"),
    ])));
    for waiter in waiters {
        assert!(matches!(
            waiter.join().expect("waiter must not panic"),
            RunTerminalKind::Failed(RunFailure::Execution(_))
        ));
    }
}

#[test]
fn abandoned_result_materialization_fails_and_wakes_future_callers() {
    let cache = Arc::new(ResultCache::new());
    assert!(matches!(cache.claim(), CacheClaim::Materialize));
    {
        let _claim = MaterializationClaim::new(Arc::clone(&cache));
    }
    let CacheClaim::Failed(diagnostics) = cache.claim() else {
        panic!("an abandoned materializer must leave one stable failure");
    };
    assert_eq!(diagnostics[0].code().to_string(), "EQ0002");
    assert_eq!(
        diagnostics[0].message(),
        "the completed native Result could not be materialized"
    );
}
