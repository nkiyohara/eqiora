use std::num::{NonZeroU64, NonZeroUsize};

/// Hardware/deployment target independent from numerical method and scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Target {
    /// Portable host CPU execution.
    HostCpu {
        /// Maximum worker threads available to the realization.
        threads: NonZeroUsize,
    },
    /// CUDA device selected by stable ordinal.
    CudaGpu {
        /// Device ordinal resolved by the deployment environment.
        device: u16,
    },
}

/// Deployment-time scheduling only.
///
/// This type deliberately has no model period, phase, event, or clock-domain
/// field. Its deadline constrains deployment execution; it does not activate a
/// semantic Relation.
///
/// Model-time activation cannot be added through this API:
///
/// ```compile_fail
/// use eqiora_realization::ExecutionSchedule;
///
/// let schedule = ExecutionSchedule::Offline;
/// schedule.with_model_period_seconds(0.01);
/// ```
///
/// Conversely, a Semantic Kernel clock cannot acquire deployment priority:
///
/// ```compile_fail
/// use eqiora_core::{Id, entity::kinds};
/// use eqiora_schema::kernel::ClockDomainDef;
///
/// let clock = ClockDomainDef::continuous(Id::<kinds::ClockDomain>::new());
/// clock.with_priority(3);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ExecutionSchedule {
    /// No real-time deployment promise.
    Offline,
    /// Priority/deadline constraints for a deployment task.
    RealTime {
        /// Platform scheduling priority; interpretation belongs to the target adapter.
        priority: u16,
        /// Non-zero deployment deadline in nanoseconds.
        deadline_ns: NonZeroU64,
    },
}
