use std::io;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::ContractError;

pub(crate) const MAX_CANONICAL_JSON_BYTES: usize = 512 * 1024 * 1024;

pub(crate) fn to_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, ContractError> {
    to_bytes_with_limit(value, MAX_CANONICAL_JSON_BYTES)
}

pub(crate) fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, ContractError> {
    from_slice_with_limit(bytes, MAX_CANONICAL_JSON_BYTES)
}

pub(crate) fn to_bytes_with_limit<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<Vec<u8>, ContractError> {
    let mut output = BoundedBytes::new(limit);
    serde_json::to_writer(&mut output, value)
        .map_err(|error| ContractError::new(format!("canonical JSON encoding failed: {error}")))?;
    Ok(output.into_inner())
}

pub(crate) fn encoded_len_with_limit<T: Serialize>(
    value: &T,
    limit: usize,
) -> Result<usize, ContractError> {
    let mut output = BoundedCount::new(limit);
    serde_json::to_writer(&mut output, value)
        .map_err(|error| ContractError::new(format!("canonical JSON encoding failed: {error}")))?;
    Ok(output.len())
}

pub(crate) fn from_slice_with_limit<T: DeserializeOwned>(
    bytes: &[u8],
    limit: usize,
) -> Result<T, ContractError> {
    if bytes.len() > limit {
        return Err(ContractError::new(format!(
            "canonical JSON exceeds the {limit}-byte wire limit"
        )));
    }
    serde_json::from_slice(bytes)
        .map_err(|error| ContractError::new(format!("canonical JSON decoding failed: {error}")))
}

struct BoundedBytes {
    bytes: Vec<u8>,
    limit: usize,
}

impl BoundedBytes {
    fn new(limit: usize) -> Self {
        Self {
            bytes: Vec::new(),
            limit,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.bytes
    }
}

impl io::Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("canonical JSON byte count overflow"))?;
        if length > self.limit {
            return Err(io::Error::other("canonical JSON wire limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

struct BoundedCount {
    bytes: usize,
    limit: usize,
}

impl BoundedCount {
    const fn new(limit: usize) -> Self {
        Self { bytes: 0, limit }
    }

    const fn len(&self) -> usize {
        self.bytes
    }
}

impl io::Write for BoundedCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let length = self
            .bytes
            .checked_add(bytes.len())
            .ok_or_else(|| io::Error::other("canonical JSON byte count overflow"))?;
        if length > self.limit {
            return Err(io::Error::other("canonical JSON wire limit exceeded"));
        }
        self.bytes = length;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(crate) fn checked_round_trip<T>(value: &T) -> Result<Vec<u8>, ContractError>
where
    T: Serialize,
{
    to_bytes(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_wire_limit_is_checked_before_decode_and_during_encode() {
        assert!(from_slice_with_limit::<Vec<u8>>(b"[0,0]", 4).is_err());
        assert!(to_bytes_with_limit(&vec![0_u8, 0], 4).is_err());
        assert!(encoded_len_with_limit(&vec![0_u8, 0], 4).is_err());
        assert_eq!(from_slice_with_limit::<Vec<u8>>(b"[0]", 4), Ok(vec![0]));
        assert_eq!(encoded_len_with_limit(&vec![0_u8], 3), Ok(3));
    }
}
