use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use eqiora_device::{
    BufferId, Completion, DeviceBufferDescriptor, DeviceExecutionTimings, DeviceId, Fence,
    HostBufferDescriptor, MemoryRegion, QueueId, QueueSlot, QueueTimeline, RuntimeId,
    SparseActionTolerance, TransferDirection, TransferEvidence, TransferPlan, WaitedCompletion,
};

const CUDA: RuntimeId = RuntimeId::new("eqiora.cuda");

fn device(ordinal: u16) -> DeviceId {
    DeviceId::new(CUDA, ordinal)
}

fn queue(device: DeviceId, slot: u32, materialization: u64) -> QueueId {
    QueueId::new(
        QueueSlot::new(device, slot),
        NonZeroU64::new(materialization).unwrap(),
    )
}

fn device_buffer<T: eqiora_device::DeviceElement>(
    ordinal: u16,
    allocation: u64,
    elements: usize,
) -> DeviceBufferDescriptor<T> {
    DeviceBufferDescriptor::new(
        BufferId::new(device(ordinal), NonZeroU64::new(allocation).unwrap()),
        NonZeroUsize::new(elements).unwrap(),
    )
}

#[test]
fn transfer_shape_direction_and_byte_count_are_explicit() {
    let host = HostBufferDescriptor::<f64>::new(NonZeroUsize::new(3).unwrap());
    let resident = device_buffer::<f64>(2, 1, 3);
    let plan = TransferPlan::new(MemoryRegion::Host(host), MemoryRegion::Device(resident)).unwrap();

    assert_eq!(plan.direction(), TransferDirection::HostToDevice);
    assert_eq!(plan.bytes().unwrap(), 24);

    let wrong_size = HostBufferDescriptor::<f64>::new(NonZeroUsize::new(2).unwrap());
    assert!(
        TransferPlan::new(
            MemoryRegion::Host(wrong_size),
            MemoryRegion::Device(resident),
        )
        .is_err()
    );
    assert!(TransferPlan::new(MemoryRegion::Host(host), MemoryRegion::Host(host)).is_err());
}

#[test]
fn queue_order_is_monotone_and_never_invented_across_queues() {
    let mut first = QueueTimeline::new(queue(device(2), 0, 1));
    let mut second = QueueTimeline::new(queue(device(2), 0, 2));
    let a = Completion::new(first.next_submission().unwrap());
    let b = Completion::new(first.next_submission().unwrap());
    let unrelated = Completion::new(second.next_submission().unwrap());

    assert!(a.happens_before(b).unwrap());
    assert!(!b.happens_before(a).unwrap());
    assert!(a.happens_before(unrelated).is_err());
}

#[derive(Debug)]
struct TestFence {
    completion: Completion,
    succeeds: bool,
}

impl Fence for TestFence {
    fn completion(&self) -> Completion {
        self.completion
    }

    fn wait(&self) -> Result<(), eqiora_core::Diagnostic> {
        self.succeeds.then_some(()).ok_or_else(|| {
            eqiora_core::Diagnostic::error(
                eqiora_core::diagnostic::codes::INVALID_REALIZATION,
                "synthetic fence failed",
            )
        })
    }
}

#[test]
fn host_visibility_exists_only_after_a_successful_fence_wait() {
    let mut timeline = QueueTimeline::new(queue(device(2), 0, 1));
    let completion = Completion::new(timeline.next_submission().unwrap());
    let visible = WaitedCompletion::wait(&TestFence {
        completion,
        succeeds: true,
    })
    .unwrap();
    assert_eq!(visible.completion(), completion);

    assert!(
        WaitedCompletion::wait(&TestFence {
            completion,
            succeeds: false,
        })
        .is_err()
    );
}

#[test]
fn transfer_evidence_requires_a_queue_on_an_endpoint_device() {
    let host = HostBufferDescriptor::<f64>::new(NonZeroUsize::MIN);
    let resident = device_buffer::<f64>(2, 1, 1);
    let plan = TransferPlan::new(MemoryRegion::Host(host), MemoryRegion::Device(resident)).unwrap();

    let mut valid_queue = QueueTimeline::new(queue(device(2), 0, 1));
    let completion = Completion::new(valid_queue.next_submission().unwrap());
    assert!(TransferEvidence::new(plan, completion).is_ok());

    let mut wrong_queue = QueueTimeline::new(queue(device(3), 0, 1));
    let completion = Completion::new(wrong_queue.next_submission().unwrap());
    assert!(TransferEvidence::new(plan, completion).is_err());
}

#[test]
fn timing_phases_may_overlap_but_cannot_exceed_total() {
    let total = Duration::from_millis(10);
    let timings = DeviceExecutionTimings::new(
        Duration::from_millis(2),
        Duration::from_millis(4),
        Duration::from_millis(7),
        Duration::from_millis(3),
        Duration::from_millis(2),
        total,
    )
    .unwrap();
    assert_eq!(timings.total(), total);

    assert!(
        DeviceExecutionTimings::new(
            Duration::from_millis(11),
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            Duration::ZERO,
            total,
        )
        .is_err()
    );
}

#[test]
fn sparse_action_tolerance_is_explicit_and_scaled_by_the_oracle() {
    let tolerance = SparseActionTolerance::new(1.0e-12, 1.0e-9).unwrap();
    assert_eq!(tolerance.threshold(2.0), 1.0e-12 + 2.0e-9);
    assert!(SparseActionTolerance::new(0.0, 0.0).is_err());
}
