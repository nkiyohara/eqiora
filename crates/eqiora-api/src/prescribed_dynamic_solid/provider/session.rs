//! Failure-atomic ownership of one already-connected child process.

use std::io::Read;
use std::process::{Child, ChildStderr, ChildStdin};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use serde::Serialize;

use super::super::invalid;
use super::protocol::control::{self, ReceivedControl};
use super::protocol::frame::{Frame, FrameKind};
use super::protocol::{Direction, Transcript};

const DEADLINE: Duration = Duration::from_secs(5);
const CANCELLATION_POLL: Duration = Duration::from_millis(10);
const STDERR_BUDGET: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CancellationCheckpoint {
    BeforeSessionAdmission,
    BeforeProviderFrame,
    WhileWaitingForProvider,
    AfterProviderFrame,
    BeforeProjection,
    AfterProjection,
    BeforeStructuralSolve,
    AfterStructuralSolve,
    BeforeComposition,
    AfterComposition,
}

#[cfg(test)]
thread_local! {
    static TEST_CANCELLATION_HOOK: std::cell::Cell<
        Option<fn(CancellationCheckpoint, &AtomicBool)>,
    > = const { std::cell::Cell::new(None) };
}

/// Install the current test thread's exact safe-boundary cancellation hook.
#[cfg(test)]
pub(super) fn set_test_cancellation_checkpoint_hook(
    hook: Option<fn(CancellationCheckpoint, &AtomicBool)>,
) {
    TEST_CANCELLATION_HOOK.set(hook);
}

pub(super) fn check_cancellation(
    cancellation: &AtomicBool,
    checkpoint: CancellationCheckpoint,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    TEST_CANCELLATION_HOOK.with(|hook| {
        if let Some(hook) = hook.get() {
            hook(checkpoint, cancellation);
        }
    });
    #[cfg(not(test))]
    let _ = checkpoint;
    if cancellation.load(Ordering::Acquire) {
        return Err(Diagnostic::error(
            codes::EXECUTION_CANCELLED,
            "prescribed dynamic-solid provider occurrence was cancelled",
        ));
    }
    Ok(())
}

enum ReadEvent {
    Frame(Frame),
    Eof,
    Failure(Diagnostic),
}

struct StderrResult {
    _diagnostic: Vec<u8>,
    overflowed: bool,
}

pub(super) struct ConnectedSession {
    child: Child,
    stdin: Option<ChildStdin>,
    receiver: Receiver<ReadEvent>,
    stdout_reader: Option<JoinHandle<()>>,
    stderr_reader: Option<JoinHandle<StderrResult>>,
    transcript: Transcript,
    clean: bool,
}

impl ConnectedSession {
    pub(super) fn admit(mut child: Child, cancellation: &AtomicBool) -> Result<Self, Diagnostic> {
        if let Err(error) =
            check_cancellation(cancellation, CancellationCheckpoint::BeforeSessionAdmission)
        {
            poison_child(&mut child);
            return Err(error);
        }
        if child.stdin.is_none() || child.stdout.is_none() {
            poison_child(&mut child);
            return Err(invalid(
                "connected provider must supply piped stdin and stdout",
            ));
        }
        let stdin = child.stdin.take().expect("provider stdin was preflighted");
        let mut stdout = child
            .stdout
            .take()
            .expect("provider stdout was preflighted");
        let (sender, receiver) = mpsc::channel();
        let stdout_reader = thread::spawn(move || {
            loop {
                let event = match Frame::read_from(&mut stdout) {
                    Ok(Some(frame)) => ReadEvent::Frame(frame),
                    Ok(None) => ReadEvent::Eof,
                    Err(error) => ReadEvent::Failure(error),
                };
                let terminal = !matches!(event, ReadEvent::Frame(_));
                if sender.send(event).is_err() || terminal {
                    break;
                }
            }
        });
        let stderr_reader = child.stderr.take().map(drain_stderr);
        Ok(Self {
            child,
            stdin: Some(stdin),
            receiver,
            stdout_reader: Some(stdout_reader),
            stderr_reader,
            transcript: Transcript::default(),
            clean: false,
        })
    }

    pub(super) fn send_control<T: Serialize>(
        &mut self,
        value: &T,
        cancellation: &AtomicBool,
    ) -> Result<(), Diagnostic> {
        self.send_frame(Frame::control(control::encode(value)?)?, cancellation)
    }

    pub(super) fn send_bulk(
        &mut self,
        bytes: Vec<u8>,
        cancellation: &AtomicBool,
    ) -> Result<(), Diagnostic> {
        self.send_frame(Frame::bulk(bytes)?, cancellation)
    }

    pub(super) fn receive_control(
        &mut self,
        cancellation: &AtomicBool,
    ) -> Result<ReceivedControl, Diagnostic> {
        let frame = self.receive_frame(FrameKind::Control, cancellation)?;
        control::decode(frame.payload())
    }

    pub(super) fn receive_bulk(
        &mut self,
        cancellation: &AtomicBool,
    ) -> Result<Vec<u8>, Diagnostic> {
        Ok(self
            .receive_frame(FrameKind::Bulk, cancellation)?
            .payload()
            .to_vec())
    }

    pub(super) const fn transcript(&self) -> &Transcript {
        &self.transcript
    }

    pub(super) fn finish(mut self, cancellation: &AtomicBool) -> Result<Transcript, Diagnostic> {
        self.stdin.take();
        match self.await_event(cancellation, "provider final EOF")? {
            ReadEvent::Eof => {}
            ReadEvent::Frame(_) => {
                return Err(invalid(
                    "provider emitted an extra frame or bytes after closed",
                ));
            }
            ReadEvent::Failure(error) => return Err(error),
        }
        let exit_deadline = Instant::now() + DEADLINE;
        let status = loop {
            check_cancellation(
                cancellation,
                CancellationCheckpoint::WhileWaitingForProvider,
            )?;
            match self.child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() < exit_deadline => thread::sleep(CANCELLATION_POLL),
                Ok(None) => return Err(invalid("provider process exit exceeded five seconds")),
                Err(error) => {
                    return Err(invalid(format!(
                        "cannot wait for provider process: {error}"
                    )));
                }
            }
        };
        if !status.success() {
            return Err(invalid("provider process did not exit successfully"));
        }
        join_stdout(self.stdout_reader.take())?;
        check_stderr(self.stderr_reader.take())?;
        self.transcript.validate_success()?;
        self.clean = true;
        Ok(std::mem::take(&mut self.transcript))
    }

    fn send_frame(&mut self, frame: Frame, cancellation: &AtomicBool) -> Result<(), Diagnostic> {
        check_cancellation(cancellation, CancellationCheckpoint::BeforeProviderFrame)?;
        frame.write_to(
            self.stdin
                .as_mut()
                .ok_or_else(|| invalid("provider stdin is already closed"))?,
        )?;
        self.transcript.record(Direction::Outgoing, &frame)?;
        check_cancellation(cancellation, CancellationCheckpoint::AfterProviderFrame)
    }

    fn receive_frame(
        &mut self,
        expected: FrameKind,
        cancellation: &AtomicBool,
    ) -> Result<Frame, Diagnostic> {
        check_cancellation(cancellation, CancellationCheckpoint::BeforeProviderFrame)?;
        let frame = match self.await_event(cancellation, "provider frame")? {
            ReadEvent::Frame(frame) => frame,
            ReadEvent::Eof => return Err(invalid("provider closed stdout before the next frame")),
            ReadEvent::Failure(error) => return Err(error),
        };
        check_cancellation(cancellation, CancellationCheckpoint::AfterProviderFrame)?;
        if frame.kind() != expected {
            return Err(invalid(
                "provider frame kind differs from the active protocol state",
            ));
        }
        self.transcript.record(Direction::Incoming, &frame)?;
        Ok(frame)
    }

    fn await_event(&self, cancellation: &AtomicBool, label: &str) -> Result<ReadEvent, Diagnostic> {
        let deadline = Instant::now() + DEADLINE;
        loop {
            check_cancellation(
                cancellation,
                CancellationCheckpoint::WhileWaitingForProvider,
            )?;
            let now = Instant::now();
            if now >= deadline {
                return Err(invalid(format!("{label} exceeded five seconds")));
            }
            let remaining = deadline.saturating_duration_since(now);
            match self.receiver.recv_timeout(remaining.min(CANCELLATION_POLL)) {
                Ok(event) => {
                    check_cancellation(
                        cancellation,
                        CancellationCheckpoint::WhileWaitingForProvider,
                    )?;
                    return Ok(event);
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    return Err(invalid("provider stdout reader ended without terminal EOF"));
                }
            }
        }
    }

    fn poison(&mut self) {
        self.stdin.take();
        poison_child(&mut self.child);
        let _ = join_stdout(self.stdout_reader.take());
        let _ = check_stderr(self.stderr_reader.take());
    }
}

impl Drop for ConnectedSession {
    fn drop(&mut self) {
        if !self.clean {
            self.poison();
        }
    }
}

fn drain_stderr(mut stderr: ChildStderr) -> JoinHandle<StderrResult> {
    thread::spawn(move || {
        let mut diagnostic = Vec::with_capacity(STDERR_BUDGET);
        let mut overflowed = false;
        let mut buffer = [0_u8; 512];
        loop {
            match stderr.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(count) => {
                    let retained = STDERR_BUDGET.saturating_sub(diagnostic.len()).min(count);
                    diagnostic.extend_from_slice(&buffer[..retained]);
                    overflowed |= retained != count;
                }
            }
        }
        StderrResult {
            _diagnostic: diagnostic,
            overflowed,
        }
    })
}

fn poison_child(child: &mut Child) {
    if child.try_wait().ok().flatten().is_none() {
        let _ = child.kill();
    }
    let _ = child.wait();
}

fn join_stdout(reader: Option<JoinHandle<()>>) -> Result<(), Diagnostic> {
    if reader.is_some_and(|reader| reader.join().is_err()) {
        return Err(invalid("provider stdout reader panicked"));
    }
    Ok(())
}

fn check_stderr(reader: Option<JoinHandle<StderrResult>>) -> Result<(), Diagnostic> {
    if let Some(reader) = reader {
        let result = reader
            .join()
            .map_err(|_| invalid("provider stderr reader panicked"))?;
        if result.overflowed {
            return Err(invalid("provider stderr exceeded the 4096-byte budget"));
        }
    }
    Ok(())
}
