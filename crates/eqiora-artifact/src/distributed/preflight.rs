use std::fmt;
use std::marker::PhantomData;

use eqiora_core::Diagnostic;
use serde::Deserialize;
use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};

use super::{DISTRIBUTED_LAYOUT_SCHEMA, LINEAR_SYSTEM_SCHEMA, PARTITION_SCHEMA};
use crate::{CANONICAL_ENCODING, DecoderLimits, invalid_artifact};

const LINEAR_SYSTEM_FIELDS: [&str; 9] = [
    "schema",
    "encoding",
    "scalar",
    "dimension",
    "row_offsets",
    "column_indices",
    "values",
    "right_hand_side",
    "properties",
];
const PARTITION_FIELDS: [&str; 6] = [
    "schema",
    "encoding",
    "scalar",
    "dimension",
    "partition_count",
    "owners",
];
const DISTRIBUTED_LAYOUT_FIELDS: [&str; 6] = [
    "schema",
    "encoding",
    "linear_system_sha256",
    "partition_sha256",
    "local_layouts",
    "halo_exchanges",
];
const LOCAL_RECORD_FIELDS: [&str; 3] = ["partition", "owned", "ghosts"];
const HALO_RECORD_FIELDS: [&str; 3] = ["owner", "receiver", "indices"];

/// Validate every distributed wire field and bound every large sequence
/// before Serde is allowed to materialize an owned DTO.
pub(super) fn linear_system(bytes: &[u8], limits: DecoderLimits) -> Result<(), Diagnostic> {
    run(bytes, LinearSystemSeed { limits }, "linear-system")
}

pub(super) fn partition(bytes: &[u8], limits: DecoderLimits) -> Result<(), Diagnostic> {
    run(bytes, PartitionSeed { limits }, "partition")
}

pub(super) fn distributed_layout(bytes: &[u8], limits: DecoderLimits) -> Result<(), Diagnostic> {
    run(
        bytes,
        DistributedLayoutSeed { limits },
        "distributed-layout",
    )
}

fn run<'de, S>(bytes: &'de [u8], seed: S, label: &str) -> Result<(), Diagnostic>
where
    S: DeserializeSeed<'de, Value = ()>,
{
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    seed.deserialize(&mut deserializer)
        .map_err(|error| invalid_artifact(format!("invalid {label} preflight: {error}")))?;
    deserializer
        .end()
        .map_err(|error| invalid_artifact(format!("invalid {label} JSON tail: {error}")))
}

#[derive(Debug)]
struct AggregateBudget {
    used: usize,
    limit: usize,
}

impl AggregateBudget {
    const fn new(limit: usize) -> Self {
        Self { used: 0, limit }
    }

    fn charge<E: de::Error>(&mut self, amount: usize) -> Result<(), E> {
        let next = self
            .used
            .checked_add(amount)
            .ok_or_else(|| E::custom("distributed aggregate work overflows usize"))?;
        if next > self.limit {
            return Err(E::custom(format_args!(
                "distributed aggregate work {next} exceeds decoder limit {}",
                self.limit
            )));
        }
        self.used = next;
        Ok(())
    }
}

struct CountSequence<'a, T> {
    label: &'static str,
    count: &'a mut usize,
    limit: usize,
    aggregate: &'a mut AggregateBudget,
    marker: PhantomData<T>,
}

impl<'a, T> CountSequence<'a, T> {
    fn new(
        label: &'static str,
        count: &'a mut usize,
        limit: usize,
        aggregate: &'a mut AggregateBudget,
    ) -> Self {
        Self {
            label,
            count,
            limit,
            aggregate,
            marker: PhantomData,
        }
    }
}

impl<'de, T> DeserializeSeed<'de> for CountSequence<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(CountSequenceVisitor { seed: self })
    }
}

struct CountSequenceVisitor<'a, T> {
    seed: CountSequence<'a, T>,
}

impl<'de, T> Visitor<'de> for CountSequenceVisitor<'_, T>
where
    T: Deserialize<'de>,
{
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "a bounded {} sequence", self.seed.label)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        loop {
            if *self.seed.count == self.seed.limit {
                sequence.next_element_seed(RejectElement {
                    label: self.seed.label,
                    limit: self.seed.limit,
                })?;
                return Ok(());
            }
            let Some(_) = sequence.next_element::<T>()? else {
                return Ok(());
            };
            *self.seed.count = self
                .seed
                .count
                .checked_add(1)
                .ok_or_else(|| de::Error::custom("sequence count overflows usize"))?;
            self.seed.aggregate.charge::<A::Error>(1)?;
        }
    }
}

struct RejectElement {
    label: &'static str,
    limit: usize,
}

impl<'de> DeserializeSeed<'de> for RejectElement {
    type Value = ();

    fn deserialize<D>(self, _deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        Err(de::Error::custom(format_args!(
            "{} count exceeds decoder limit {}",
            self.label, self.limit
        )))
    }
}

struct CanonicalF64;

impl<'de> Deserialize<'de> for CanonicalF64 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_f64(CanonicalF64Visitor)
    }
}

struct CanonicalF64Visitor;

impl Visitor<'_> for CanonicalF64Visitor {
    type Value = CanonicalF64;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a finite f64 using canonical positive zero")
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if !value.is_finite() {
            return Err(E::custom("distributed f64 value must be finite"));
        }
        if value == 0.0 && value.is_sign_negative() {
            return Err(E::custom(
                "distributed f64 value must use canonical positive zero",
            ));
        }
        Ok(CanonicalF64)
    }
}

struct PortableIndex;

impl<'de> Deserialize<'de> for PortableIndex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = u64::deserialize(deserializer)?;
        usize::try_from(value)
            .map(|_| PortableIndex)
            .map_err(|_| de::Error::custom("distributed index exceeds local usize"))
    }
}

fn mark<E: de::Error>(seen: &mut bool, label: &'static str) -> Result<(), E> {
    if *seen {
        Err(E::duplicate_field(label))
    } else {
        *seen = true;
        Ok(())
    }
}

fn require_fields<E: de::Error>(seen: &[bool], fields: &'static [&'static str]) -> Result<(), E> {
    for (&present, &field) in seen.iter().zip(fields) {
        if !present {
            return Err(E::missing_field(field));
        }
    }
    Ok(())
}

fn reject_unknown<E: de::Error, T>(field: &str, fields: &'static [&'static str]) -> Result<T, E> {
    Err(E::unknown_field(field, fields))
}

fn exact_value<E: de::Error>(value: &str, expected: &'static str, label: &str) -> Result<(), E> {
    if value == expected {
        Ok(())
    } else {
        Err(E::custom(format_args!(
            "unsupported {label} value `{value}`; expected `{expected}`"
        )))
    }
}

fn canonical_digest<E: de::Error>(value: &str, label: &str) -> Result<(), E> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(E::custom(format_args!(
            "{label} must be 64 lowercase hexadecimal SHA-256 characters"
        )))
    }
}

fn portable_bounded<E: de::Error>(value: u64, limit: usize, label: &str) -> Result<usize, E> {
    let value = usize::try_from(value)
        .map_err(|_| E::custom(format_args!("{label} exceeds local usize")))?;
    if value > limit {
        return Err(E::custom(format_args!(
            "{label} {value} exceeds decoder limit {limit}"
        )));
    }
    Ok(value)
}

struct LinearSystemSeed {
    limits: DecoderLimits,
}

impl<'de> DeserializeSeed<'de> for LinearSystemSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(LinearSystemVisitor {
            limits: self.limits,
        })
    }
}

struct LinearSystemVisitor {
    limits: DecoderLimits,
}

impl<'de> Visitor<'de> for LinearSystemVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete linear-system envelope object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut aggregate = AggregateBudget::new(self.limits.max_distributed_aggregate_work);
        let mut seen = [false; LINEAR_SYSTEM_FIELDS.len()];
        let mut dimension = None;
        let mut row_offsets = 0;
        let mut columns = 0;
        let mut values = 0;
        let mut right_hand_side = 0;
        let row_limit = self.limits.max_distributed_dimension.saturating_add(1);

        while let Some(field) = map.next_key::<&'de str>()? {
            match field {
                "schema" => {
                    mark::<A::Error>(&mut seen[0], "schema")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, LINEAR_SYSTEM_SCHEMA, "linear-system schema")?;
                }
                "encoding" => {
                    mark::<A::Error>(&mut seen[1], "encoding")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, CANONICAL_ENCODING, "canonical encoding")?;
                }
                "scalar" => {
                    mark::<A::Error>(&mut seen[2], "scalar")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, "f64", "linear-system scalar")?;
                }
                "dimension" => {
                    mark::<A::Error>(&mut seen[3], "dimension")?;
                    let value = portable_bounded::<A::Error>(
                        map.next_value()?,
                        self.limits.max_distributed_dimension,
                        "linear-system dimension",
                    )?;
                    if value == 0 {
                        return Err(de::Error::custom("linear-system dimension must be nonzero"));
                    }
                    aggregate.charge::<A::Error>(value)?;
                    dimension = Some(value);
                }
                "row_offsets" => {
                    mark::<A::Error>(&mut seen[4], "row_offsets")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "linear-system row-offset",
                        &mut row_offsets,
                        row_limit,
                        &mut aggregate,
                    ))?;
                }
                "column_indices" => {
                    mark::<A::Error>(&mut seen[5], "column_indices")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "linear-system nonzero",
                        &mut columns,
                        self.limits.max_distributed_nonzeros,
                        &mut aggregate,
                    ))?;
                }
                "values" => {
                    mark::<A::Error>(&mut seen[6], "values")?;
                    map.next_value_seed(CountSequence::<CanonicalF64>::new(
                        "linear-system value",
                        &mut values,
                        self.limits.max_distributed_nonzeros,
                        &mut aggregate,
                    ))?;
                }
                "right_hand_side" => {
                    mark::<A::Error>(&mut seen[7], "right_hand_side")?;
                    map.next_value_seed(CountSequence::<CanonicalF64>::new(
                        "linear-system right-hand-side",
                        &mut right_hand_side,
                        self.limits.max_distributed_dimension,
                        &mut aggregate,
                    ))?;
                }
                "properties" => {
                    mark::<A::Error>(&mut seen[8], "properties")?;
                    let value: &'de str = map.next_value()?;
                    if value != "general" && value != "symmetric-positive-definite" {
                        return Err(de::Error::custom(format_args!(
                            "unsupported linear-system properties value `{value}`"
                        )));
                    }
                }
                other => return reject_unknown(other, &LINEAR_SYSTEM_FIELDS),
            }
        }

        require_fields::<A::Error>(&seen, &LINEAR_SYSTEM_FIELDS)?;
        let dimension = dimension.ok_or_else(|| de::Error::missing_field("dimension"))?;
        if row_offsets != dimension.saturating_add(1) || right_hand_side != dimension {
            return Err(de::Error::custom(
                "linear-system row offsets and right-hand side contradict its dimension",
            ));
        }
        if columns != values {
            return Err(de::Error::custom(
                "linear-system columns and values must have equal length",
            ));
        }
        Ok(())
    }
}

struct PartitionSeed {
    limits: DecoderLimits,
}

impl<'de> DeserializeSeed<'de> for PartitionSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(PartitionVisitor {
            limits: self.limits,
        })
    }
}

struct PartitionVisitor {
    limits: DecoderLimits,
}

impl<'de> Visitor<'de> for PartitionVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete partition envelope object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut aggregate = AggregateBudget::new(self.limits.max_distributed_aggregate_work);
        let mut seen = [false; PARTITION_FIELDS.len()];
        let mut dimension = None;
        let mut owners = 0;

        while let Some(field) = map.next_key::<&'de str>()? {
            match field {
                "schema" => {
                    mark::<A::Error>(&mut seen[0], "schema")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, PARTITION_SCHEMA, "partition schema")?;
                }
                "encoding" => {
                    mark::<A::Error>(&mut seen[1], "encoding")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, CANONICAL_ENCODING, "canonical encoding")?;
                }
                "scalar" => {
                    mark::<A::Error>(&mut seen[2], "scalar")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, "f64", "partition scalar")?;
                }
                "dimension" => {
                    mark::<A::Error>(&mut seen[3], "dimension")?;
                    let value = portable_bounded::<A::Error>(
                        map.next_value()?,
                        self.limits.max_distributed_dimension,
                        "partition dimension",
                    )?;
                    if value == 0 {
                        return Err(de::Error::custom("partition dimension must be nonzero"));
                    }
                    aggregate.charge::<A::Error>(value)?;
                    dimension = Some(value);
                }
                "partition_count" => {
                    mark::<A::Error>(&mut seen[4], "partition_count")?;
                    let value = portable_bounded::<A::Error>(
                        map.next_value()?,
                        self.limits.max_distributed_partitions,
                        "partition count",
                    )?;
                    if value == 0 {
                        return Err(de::Error::custom("partition count must be nonzero"));
                    }
                    aggregate.charge::<A::Error>(value)?;
                }
                "owners" => {
                    mark::<A::Error>(&mut seen[5], "owners")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "partition owner-map entry",
                        &mut owners,
                        self.limits.max_distributed_owner_entries,
                        &mut aggregate,
                    ))?;
                }
                other => return reject_unknown(other, &PARTITION_FIELDS),
            }
        }

        require_fields::<A::Error>(&seen, &PARTITION_FIELDS)?;
        let dimension = dimension.ok_or_else(|| de::Error::missing_field("dimension"))?;
        if owners != dimension {
            return Err(de::Error::custom(
                "partition owner map must contain one entry per global index",
            ));
        }
        Ok(())
    }
}

struct DistributedLayoutSeed {
    limits: DecoderLimits,
}

impl<'de> DeserializeSeed<'de> for DistributedLayoutSeed {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(DistributedLayoutVisitor {
            limits: self.limits,
        })
    }
}

struct DistributedLayoutVisitor {
    limits: DecoderLimits,
}

impl<'de> Visitor<'de> for DistributedLayoutVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a complete distributed-layout envelope object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut aggregate = AggregateBudget::new(self.limits.max_distributed_aggregate_work);
        let mut seen = [false; DISTRIBUTED_LAYOUT_FIELDS.len()];
        let mut local_records = 0;
        let mut local_indices = 0;
        let mut halo_records = 0;
        let mut halo_indices = 0;

        while let Some(field) = map.next_key::<&'de str>()? {
            match field {
                "schema" => {
                    mark::<A::Error>(&mut seen[0], "schema")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, DISTRIBUTED_LAYOUT_SCHEMA, "layout schema")?;
                }
                "encoding" => {
                    mark::<A::Error>(&mut seen[1], "encoding")?;
                    let value: &'de str = map.next_value()?;
                    exact_value::<A::Error>(value, CANONICAL_ENCODING, "canonical encoding")?;
                }
                "linear_system_sha256" => {
                    mark::<A::Error>(&mut seen[2], "linear_system_sha256")?;
                    let value: &'de str = map.next_value()?;
                    canonical_digest::<A::Error>(value, "linear-system digest")?;
                }
                "partition_sha256" => {
                    mark::<A::Error>(&mut seen[3], "partition_sha256")?;
                    let value: &'de str = map.next_value()?;
                    canonical_digest::<A::Error>(value, "partition digest")?;
                }
                "local_layouts" => {
                    mark::<A::Error>(&mut seen[4], "local_layouts")?;
                    map.next_value_seed(LocalRecordsSeed {
                        records: &mut local_records,
                        local_indices: &mut local_indices,
                        aggregate: &mut aggregate,
                        limits: self.limits,
                    })?;
                }
                "halo_exchanges" => {
                    mark::<A::Error>(&mut seen[5], "halo_exchanges")?;
                    map.next_value_seed(HaloRecordsSeed {
                        records: &mut halo_records,
                        halo_indices: &mut halo_indices,
                        aggregate: &mut aggregate,
                        limits: self.limits,
                    })?;
                }
                other => return reject_unknown(other, &DISTRIBUTED_LAYOUT_FIELDS),
            }
        }

        require_fields::<A::Error>(&seen, &DISTRIBUTED_LAYOUT_FIELDS)?;
        if local_records == 0 {
            return Err(de::Error::custom(
                "distributed layout must contain at least one local record",
            ));
        }
        Ok(())
    }
}

struct LocalRecordsSeed<'a> {
    records: &'a mut usize,
    local_indices: &'a mut usize,
    aggregate: &'a mut AggregateBudget,
    limits: DecoderLimits,
}

impl<'de> DeserializeSeed<'de> for LocalRecordsSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(LocalRecordsVisitor { seed: self })
    }
}

struct LocalRecordsVisitor<'a> {
    seed: LocalRecordsSeed<'a>,
}

impl<'de> Visitor<'de> for LocalRecordsVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded complete local-layout records")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        loop {
            if *self.seed.records == self.seed.limits.max_distributed_partitions {
                sequence.next_element_seed(RejectElement {
                    label: "distributed local-layout record",
                    limit: self.seed.limits.max_distributed_partitions,
                })?;
                return Ok(());
            }
            let record = LocalRecordSeed {
                expected_partition: *self.seed.records,
                local_indices: &mut *self.seed.local_indices,
                aggregate: &mut *self.seed.aggregate,
                limit: self.seed.limits.max_distributed_local_indices,
            };
            if sequence.next_element_seed(record)?.is_none() {
                return Ok(());
            }
            *self.seed.records =
                self.seed.records.checked_add(1).ok_or_else(|| {
                    de::Error::custom("local-layout record count overflows usize")
                })?;
            self.seed.aggregate.charge::<A::Error>(1)?;
        }
    }
}

struct LocalRecordSeed<'a> {
    expected_partition: usize,
    local_indices: &'a mut usize,
    aggregate: &'a mut AggregateBudget,
    limit: usize,
}

impl<'de> DeserializeSeed<'de> for LocalRecordSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(LocalRecordVisitor { seed: self })
    }
}

struct LocalRecordVisitor<'a> {
    seed: LocalRecordSeed<'a>,
}

impl<'de> Visitor<'de> for LocalRecordVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one complete local-layout record")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = [false; LOCAL_RECORD_FIELDS.len()];
        let mut partition = None;
        while let Some(field) = map.next_key::<&'de str>()? {
            match field {
                "partition" => {
                    mark::<A::Error>(&mut seen[0], "partition")?;
                    let value: u64 = map.next_value()?;
                    partition =
                        Some(usize::try_from(value).map_err(|_| {
                            de::Error::custom("layout partition exceeds local usize")
                        })?);
                }
                "owned" => {
                    mark::<A::Error>(&mut seen[1], "owned")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "distributed local index",
                        &mut *self.seed.local_indices,
                        self.seed.limit,
                        &mut *self.seed.aggregate,
                    ))?;
                }
                "ghosts" => {
                    mark::<A::Error>(&mut seen[2], "ghosts")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "distributed local index",
                        &mut *self.seed.local_indices,
                        self.seed.limit,
                        &mut *self.seed.aggregate,
                    ))?;
                }
                other => return reject_unknown(other, &LOCAL_RECORD_FIELDS),
            }
        }
        require_fields::<A::Error>(&seen, &LOCAL_RECORD_FIELDS)?;
        if partition != Some(self.seed.expected_partition) {
            return Err(de::Error::custom(
                "local-layout records must use their partition-order index",
            ));
        }
        Ok(())
    }
}

struct HaloRecordsSeed<'a> {
    records: &'a mut usize,
    halo_indices: &'a mut usize,
    aggregate: &'a mut AggregateBudget,
    limits: DecoderLimits,
}

impl<'de> DeserializeSeed<'de> for HaloRecordsSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_seq(HaloRecordsVisitor { seed: self })
    }
}

struct HaloRecordsVisitor<'a> {
    seed: HaloRecordsSeed<'a>,
}

impl<'de> Visitor<'de> for HaloRecordsVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("bounded complete halo-exchange records")
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        loop {
            if *self.seed.records == self.seed.limits.max_distributed_halo_records {
                sequence.next_element_seed(RejectElement {
                    label: "distributed halo record",
                    limit: self.seed.limits.max_distributed_halo_records,
                })?;
                return Ok(());
            }
            let record = HaloRecordSeed {
                halo_indices: &mut *self.seed.halo_indices,
                aggregate: &mut *self.seed.aggregate,
                limit: self.seed.limits.max_distributed_halo_indices,
            };
            if sequence.next_element_seed(record)?.is_none() {
                return Ok(());
            }
            *self.seed.records = self
                .seed
                .records
                .checked_add(1)
                .ok_or_else(|| de::Error::custom("halo record count overflows usize"))?;
            self.seed.aggregate.charge::<A::Error>(1)?;
        }
    }
}

struct HaloRecordSeed<'a> {
    halo_indices: &'a mut usize,
    aggregate: &'a mut AggregateBudget,
    limit: usize,
}

impl<'de> DeserializeSeed<'de> for HaloRecordSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(HaloRecordVisitor { seed: self })
    }
}

struct HaloRecordVisitor<'a> {
    seed: HaloRecordSeed<'a>,
}

impl<'de> Visitor<'de> for HaloRecordVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one complete halo-exchange record")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut seen = [false; HALO_RECORD_FIELDS.len()];
        let mut owner = None;
        let mut receiver = None;
        while let Some(field) = map.next_key::<&'de str>()? {
            match field {
                "owner" => {
                    mark::<A::Error>(&mut seen[0], "owner")?;
                    let value: u64 = map.next_value()?;
                    owner = Some(
                        usize::try_from(value)
                            .map_err(|_| de::Error::custom("halo owner exceeds local usize"))?,
                    );
                }
                "receiver" => {
                    mark::<A::Error>(&mut seen[1], "receiver")?;
                    let value: u64 = map.next_value()?;
                    receiver = Some(
                        usize::try_from(value)
                            .map_err(|_| de::Error::custom("halo receiver exceeds local usize"))?,
                    );
                }
                "indices" => {
                    mark::<A::Error>(&mut seen[2], "indices")?;
                    map.next_value_seed(CountSequence::<PortableIndex>::new(
                        "distributed halo index",
                        &mut *self.seed.halo_indices,
                        self.seed.limit,
                        &mut *self.seed.aggregate,
                    ))?;
                }
                other => return reject_unknown(other, &HALO_RECORD_FIELDS),
            }
        }
        require_fields::<A::Error>(&seen, &HALO_RECORD_FIELDS)?;
        if owner == receiver {
            return Err(de::Error::custom(
                "halo owner and receiver must be distinct",
            ));
        }
        Ok(())
    }
}
