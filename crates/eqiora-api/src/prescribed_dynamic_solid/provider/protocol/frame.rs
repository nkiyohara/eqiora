//! Fixed-prefix framing and allocation preflight.

use std::io::{self, Read, Write};

use eqiora_core::Diagnostic;

use super::super::super::invalid;
use super::control::MAX_CONTROL_BYTES;

const MAGIC: [u8; 4] = *b"EQP1";
pub(in super::super) const PREFIX_BYTES: usize = 16;
pub(in super::super) const BULK_BYTES: usize = 96;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in super::super) enum FrameKind {
    Control,
    Bulk,
}

impl FrameKind {
    const fn code(self) -> u8 {
        match self {
            Self::Control => 0x01,
            Self::Bulk => 0x02,
        }
    }

    fn from_code(code: u8) -> Result<Self, Diagnostic> {
        match code {
            0x01 => Ok(Self::Control),
            0x02 => Ok(Self::Bulk),
            _ => Err(invalid("provider frame has an unknown kind")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in super::super) struct Frame {
    kind: FrameKind,
    payload: Vec<u8>,
}

impl Frame {
    pub(in super::super) fn control(payload: Vec<u8>) -> Result<Self, Diagnostic> {
        if payload.len() > MAX_CONTROL_BYTES {
            return Err(invalid("provider control frame exceeds its payload budget"));
        }
        Ok(Self {
            kind: FrameKind::Control,
            payload,
        })
    }

    pub(in super::super) fn bulk(payload: Vec<u8>) -> Result<Self, Diagnostic> {
        if payload.len() != BULK_BYTES {
            return Err(invalid("provider bulk frame must contain exactly 96 bytes"));
        }
        Ok(Self {
            kind: FrameKind::Bulk,
            payload,
        })
    }

    pub(in super::super) const fn kind(&self) -> FrameKind {
        self.kind
    }

    pub(in super::super) fn payload(&self) -> &[u8] {
        &self.payload
    }

    pub(in super::super) fn prefix(&self) -> [u8; PREFIX_BYTES] {
        let mut prefix = [0_u8; PREFIX_BYTES];
        prefix[..4].copy_from_slice(&MAGIC);
        prefix[4] = self.kind.code();
        prefix[8..].copy_from_slice(&(self.payload.len() as u64).to_le_bytes());
        prefix
    }

    pub(in super::super) fn write_to(&self, writer: &mut impl Write) -> Result<(), Diagnostic> {
        writer
            .write_all(&self.prefix())
            .and_then(|()| writer.write_all(&self.payload))
            .and_then(|()| writer.flush())
            .map_err(|error| invalid(format!("cannot write provider frame: {error}")))
    }

    pub(in super::super) fn read_from(reader: &mut impl Read) -> Result<Option<Self>, Diagnostic> {
        let mut prefix = [0_u8; PREFIX_BYTES];
        match reader.read(&mut prefix[..1]) {
            Ok(0) => return Ok(None),
            Ok(1) => {}
            Ok(_) => unreachable!("one-byte read cannot return more than one byte"),
            Err(error) => return Err(io_error("cannot read provider frame prefix", error)),
        }
        reader
            .read_exact(&mut prefix[1..])
            .map_err(|error| io_error("truncated provider frame prefix", error))?;
        if prefix[..4] != MAGIC || prefix[5..8] != [0, 0, 0] {
            return Err(invalid(
                "provider frame magic or reserved bytes are invalid",
            ));
        }
        let kind = FrameKind::from_code(prefix[4])?;
        let length = u64::from_le_bytes(
            prefix[8..]
                .try_into()
                .expect("frame prefix contains an eight-byte length"),
        );
        let length = usize::try_from(length)
            .map_err(|_| invalid("provider frame length is not portable to usize"))?;
        match kind {
            FrameKind::Control if length > MAX_CONTROL_BYTES => {
                return Err(invalid("provider control frame exceeds its payload budget"));
            }
            FrameKind::Bulk if length != BULK_BYTES => {
                return Err(invalid("provider bulk frame must contain exactly 96 bytes"));
            }
            _ => {}
        }
        let mut payload = vec![0_u8; length];
        reader
            .read_exact(&mut payload)
            .map_err(|error| io_error("truncated provider frame payload", error))?;
        Ok(Some(Self { kind, payload }))
    }
}

fn io_error(context: &str, error: io::Error) -> Diagnostic {
    invalid(format!("{context}: {error}"))
}
