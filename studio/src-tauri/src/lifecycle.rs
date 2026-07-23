//! Run-owned cancellation and coalesced accepted-step progress.
//!
//! This module has no Tauri or presentation types. The command adapter binds
//! its progress sink to an IPC channel while the shared Eqiora API remains the
//! sole execution authority.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use eqiora::api::{ReferenceRunDirective, ReferenceRunObserver, ReferenceRunProgress};

const MIN_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
struct ActiveRun {
    id: String,
    cancellation: Option<Arc<AtomicBool>>,
}

/// At most one run owned by this Studio session.
#[derive(Debug, Default)]
pub(super) struct RunRegistry {
    active: Option<ActiveRun>,
}

impl RunRegistry {
    pub(super) fn begin_cancellable(&mut self, id: String) -> Result<Arc<AtomicBool>, &str> {
        if self.active.is_some() {
            return Err("another native run is already active");
        }
        let cancellation = Arc::new(AtomicBool::new(false));
        self.active = Some(ActiveRun {
            id,
            cancellation: Some(Arc::clone(&cancellation)),
        });
        Ok(cancellation)
    }

    pub(super) fn begin_non_cancellable(&mut self, id: String) -> Result<(), &str> {
        if self.active.is_some() {
            return Err("another native run is already active");
        }
        self.active = Some(ActiveRun {
            id,
            cancellation: None,
        });
        Ok(())
    }

    pub(super) fn cancel(&self, id: &str) -> CancellationStatus {
        let Some(active) = &self.active else {
            return CancellationStatus::AlreadyTerminal;
        };
        if active.id != id {
            return CancellationStatus::AlreadyTerminal;
        }
        let Some(cancellation) = &active.cancellation else {
            return CancellationStatus::NotCancellable;
        };
        cancellation.store(true, Ordering::Release);
        CancellationStatus::Requested
    }

    pub(super) fn finish(&mut self, id: &str) {
        if self.active.as_ref().is_some_and(|active| active.id == id) {
            self.active = None;
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CancellationStatus {
    Requested,
    AlreadyTerminal,
    NotCancellable,
}

/// Converts accepted-step observations into a bounded presentation stream.
pub(super) struct CoalescingObserver<F> {
    cancellation: Arc<AtomicBool>,
    started: Instant,
    last_emitted: Option<Instant>,
    emit: F,
}

impl<F> CoalescingObserver<F> {
    pub(super) fn new(cancellation: Arc<AtomicBool>, emit: F) -> Self {
        Self {
            cancellation,
            started: Instant::now(),
            last_emitted: None,
            emit,
        }
    }
}

impl<F> ReferenceRunObserver for CoalescingObserver<F>
where
    F: FnMut(ReferenceRunProgress, Duration),
{
    fn observe(&mut self, progress: ReferenceRunProgress) -> ReferenceRunDirective {
        let now = Instant::now();
        let cancelled = self.cancellation.load(Ordering::Acquire);
        let should_emit = cancelled
            || self
                .last_emitted
                .is_none_or(|last| now.duration_since(last) >= MIN_PROGRESS_INTERVAL);
        if should_emit {
            (self.emit)(progress, now.duration_since(self.started));
            self.last_emitted = Some(now);
        }
        if cancelled {
            ReferenceRunDirective::Cancel
        } else {
            ReferenceRunDirective::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::*;

    #[test]
    fn registry_cancels_only_the_exact_active_run() {
        let mut registry = RunRegistry::default();
        let cancellation = registry.begin_cancellable("run-a".to_owned()).unwrap();
        assert!(registry.begin_cancellable("run-b".to_owned()).is_err());
        assert_eq!(
            registry.cancel("run-b"),
            CancellationStatus::AlreadyTerminal
        );
        assert!(!cancellation.load(Ordering::Acquire));

        assert_eq!(registry.cancel("run-a"), CancellationStatus::Requested);
        assert!(cancellation.load(Ordering::Acquire));
        registry.finish("run-a");
        assert_eq!(
            registry.cancel("run-a"),
            CancellationStatus::AlreadyTerminal
        );
        assert!(registry.begin_cancellable("run-b".to_owned()).is_ok());
    }

    #[test]
    fn registry_serializes_non_cancellable_work_without_faking_preemption() {
        let mut registry = RunRegistry::default();
        registry
            .begin_non_cancellable("spatial-a".to_owned())
            .unwrap();
        assert_eq!(
            registry.cancel("spatial-a"),
            CancellationStatus::NotCancellable
        );
        assert!(registry.begin_cancellable("run-b".to_owned()).is_err());
        registry.finish("spatial-a");
        assert!(registry.begin_cancellable("run-b".to_owned()).is_ok());
    }
}
