use std::num::NonZeroUsize;

use eqiora_assembly::AssemblyPlan;
use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;
use sha2::{Digest, Sha256};

pub(super) fn target_sizes(plan: &AssemblyPlan) -> Result<Vec<usize>, Diagnostic> {
    (0..plan.target_count())
        .map(|index| {
            plan.target_id(index)
                .and_then(|id| plan.target(id))
                .map(|target| target.size())
                .ok_or_else(|| invalid("spatial assembly plan target is unavailable"))
        })
        .collect()
}

pub(super) fn nonzero(value: usize, name: &'static str) -> Result<NonZeroUsize, Diagnostic> {
    NonZeroUsize::new(value).ok_or_else(|| invalid(format!("{name} must be nonzero")))
}

pub(super) fn reserve<T>(capacity: usize, name: &'static str) -> Result<Vec<T>, Diagnostic> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(capacity)
        .map_err(|_| invalid(format!("could not reserve {name}")))?;
    Ok(values)
}

pub(super) fn push_usize(bytes: &mut Vec<u8>, value: usize) -> Result<(), Diagnostic> {
    let value =
        u64::try_from(value).map_err(|_| invalid("assembly wire index exceeds portable u64"))?;
    bytes.extend_from_slice(&value.to_be_bytes());
    Ok(())
}

pub(super) struct WireReader<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> WireReader<'a> {
    pub(super) fn new(bytes: &'a [u8], magic: &[u8; 8]) -> Result<Self, Diagnostic> {
        if bytes.len() < magic.len() || &bytes[..magic.len()] != magic {
            return Err(invalid("assembly wire payload has invalid magic"));
        }
        Ok(Self {
            bytes,
            cursor: magic.len(),
        })
    }

    pub(super) fn remaining(&self) -> usize {
        self.bytes.len() - self.cursor
    }

    pub(super) fn word(&mut self) -> Result<[u8; 8], Diagnostic> {
        let end = self
            .cursor
            .checked_add(8)
            .ok_or_else(|| invalid("assembly wire cursor overflow"))?;
        let word = self
            .bytes
            .get(self.cursor..end)
            .ok_or_else(|| invalid("assembly wire payload is truncated"))?;
        self.cursor = end;
        Ok(word
            .try_into()
            .expect("an eight-byte slice always converts to an eight-byte array"))
    }

    pub(super) fn usize(&mut self) -> Result<usize, Diagnostic> {
        usize::try_from(u64::from_be_bytes(self.word()?))
            .map_err(|_| invalid("assembly wire index exceeds local usize"))
    }

    pub(super) fn f64(&mut self) -> Result<f64, Diagnostic> {
        let value = f64::from_bits(u64::from_be_bytes(self.word()?));
        value
            .is_finite()
            .then_some(value)
            .ok_or_else(|| invalid("assembly wire payload contains a non-finite value"))
    }

    pub(super) fn array_32(&mut self) -> Result<[u8; 32], Diagnostic> {
        let end = self
            .cursor
            .checked_add(32)
            .ok_or_else(|| invalid("assembly wire cursor overflow"))?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or_else(|| invalid("assembly wire payload is truncated"))?;
        self.cursor = end;
        Ok(value
            .try_into()
            .expect("a 32-byte slice always converts to a 32-byte array"))
    }

    pub(super) fn finish(self) -> Result<(), Diagnostic> {
        if self.cursor == self.bytes.len() {
            Ok(())
        } else {
            Err(invalid("assembly wire payload has trailing bytes"))
        }
    }
}

pub(super) fn hash_usizes(hash: &mut Sha256, values: &[usize]) -> Result<(), Diagnostic> {
    hash_usize(hash, values.len())?;
    for value in values {
        hash_usize(hash, *value)?;
    }
    Ok(())
}

pub(super) fn hash_f64s(hash: &mut Sha256, values: &[f64]) -> Result<(), Diagnostic> {
    hash_usize(hash, values.len())?;
    for value in values {
        hash_f64(hash, *value)?;
    }
    Ok(())
}

pub(super) fn hash_f64(hash: &mut Sha256, value: f64) -> Result<(), Diagnostic> {
    if !value.is_finite() {
        return Err(invalid("spatial assembly identity requires finite values"));
    }
    hash.update(value.to_bits().to_le_bytes());
    Ok(())
}

pub(super) fn hash_usize(hash: &mut Sha256, value: usize) -> Result<(), Diagnostic> {
    let value = u64::try_from(value)
        .map_err(|_| invalid("spatial assembly identity value exceeds portable u64"))?;
    hash.update(value.to_le_bytes());
    Ok(())
}

pub(super) fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::ASSEMBLY_FAILED, message)
}
