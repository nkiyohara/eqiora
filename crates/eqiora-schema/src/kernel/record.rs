//! Nominal heterogeneous products over existing typed values and field symbols.
use eqiora_core::{
    Diagnostic, EntityKind, Id, RawId, ValueLiteral, ValueType, diagnostic::codes, entity::kinds,
};
use std::collections::BTreeSet;

/// One exact closed record declaration with ordered heterogeneous member types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordDef {
    id: Id<kinds::Record>,
    members: Vec<(String, ValueType)>,
}
impl RecordDef {
    /// Maximum admitted members of one flat record.
    pub const MAX_MEMBERS: usize = 65_536;

    /// Check a complete nonempty record declaration before publishing its identity.
    pub fn new(
        id: Id<kinds::Record>,
        members: Vec<(String, ValueType)>,
    ) -> Result<Self, Diagnostic> {
        if members.is_empty() || members.len() > Self::MAX_MEMBERS {
            return Err(invalid("record requires between 1 and 65536 members"));
        }
        let mut seen = BTreeSet::new();
        for (name, _) in &members {
            let mut bytes = name.bytes();
            if name == "_"
                || !matches!(bytes.next(), Some(first) if first.is_ascii_alphabetic() || first == b'_')
                || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
                || !seen.insert(name)
            {
                return Err(invalid(
                    "record requires unique non-wildcard identifier members",
                ));
            }
        }
        Ok(Self { id, members })
    }
    /// Exact declaration identity, independent of member display names.
    pub const fn id(&self) -> Id<kinds::Record> {
        self.id
    }
    /// Ordered member names and complete mathematical types.
    pub fn members(&self) -> &[(String, ValueType)] {
        &self.members
    }

    /// Admit a closed static value, canonically ordering supplied named members.
    pub fn value(&self, members: Vec<(String, ValueLiteral)>) -> Result<RecordValue, Diagnostic> {
        if members.len() != self.members.len() {
            return Err(invalid(
                "record value requires every declared member exactly once",
            ));
        }
        let mut named = std::collections::BTreeMap::new();
        for (name, value) in members {
            if named.insert(name, value).is_some() {
                return Err(invalid("record value contains a duplicate member"));
            }
        }
        let mut ordered = Vec::with_capacity(self.members.len());
        for (name, value_type) in &self.members {
            let value = named
                .remove(name)
                .ok_or_else(|| invalid("record value has a missing or foreign member"))?;
            if value.value_type() != value_type {
                return Err(invalid(
                    "record member requires its exact declared mathematical type",
                ));
            }
            ordered.push(value);
        }
        Ok(RecordValue {
            definition: self.id,
            members: ordered,
        })
    }
}

/// One closed static product value; numeric and discrete member values remain distinct.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordValue {
    definition: Id<kinds::Record>,
    members: Vec<ValueLiteral>,
}
impl RecordValue {
    /// Exact owner declaration; equal labels do not substitute for this identity.
    pub const fn definition(&self) -> Id<kinds::Record> {
        self.definition
    }
    /// Values in declaration order, retaining each leaf's full mathematical type.
    pub fn members(&self) -> &[ValueLiteral] {
        &self.members
    }
}

/// An occurrence-owned product bound to exact existing Field or Parameter members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordInstanceDef {
    id: Id<kinds::RecordInstance>,
    definition: Id<kinds::Record>,
    members: Vec<RawId>,
}
impl RecordInstanceDef {
    /// Construct ordered leaf identities; whole-Model admission checks the declaration,
    /// complete member types and one shared exact temporal ownership.
    pub fn new(
        id: Id<kinds::RecordInstance>,
        definition: Id<kinds::Record>,
        members: Vec<RawId>,
    ) -> Result<Self, Diagnostic> {
        if members.is_empty() || members.len() > RecordDef::MAX_MEMBERS {
            return Err(invalid(
                "record instance requires between 1 and 65536 members",
            ));
        }
        let mut seen = BTreeSet::new();
        for member in &members {
            if !matches!(member.kind(), EntityKind::Field | EntityKind::Parameter)
                || !seen.insert(*member)
            {
                return Err(invalid(
                    "record instance requires distinct exact Field or Parameter identities",
                ));
            }
        }
        Ok(Self {
            id,
            definition,
            members,
        })
    }
    /// Exact occurrence identity.
    pub const fn id(&self) -> Id<kinds::RecordInstance> {
        self.id
    }
    /// Exact declaration owner.
    pub const fn definition(&self) -> Id<kinds::Record> {
        self.definition
    }
    /// Ordered exact member identities; members are never inferred from display names.
    pub fn members(&self) -> &[RawId] {
        &self.members
    }
}
fn invalid(message: &str) -> Diagnostic {
    Diagnostic::error(codes::INVALID_KERNEL_DEFINITION, message)
}
impl From<RecordDef> for super::KernelNode {
    fn from(value: RecordDef) -> Self {
        Self::Record(value)
    }
}
impl From<RecordInstanceDef> for super::KernelNode {
    fn from(value: RecordInstanceDef) -> Self {
        Self::RecordInstance(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use eqiora_core::{DimExponents, ScalarDomain};

    #[test]
    fn closed_values_keep_nominal_identity_member_order_dimensions_and_discrete_domains() {
        let volts = ValueType::scalar(
            ScalarDomain::Real,
            DimExponents::from_integers([1, 2, -3, -1, 0, 0, 0]).unwrap(),
        )
        .unwrap();
        let mode = super::super::EnumDef::new(Id::new(), ["Off".into(), "On".into()]).unwrap();
        let members = vec![
            ("voltage".into(), volts.clone()),
            ("mode".into(), mode.value_type()),
            ("valid".into(), ValueType::boolean()),
        ];
        let record = RecordDef::new(Id::new(), members.clone()).unwrap();
        let value = vec![
            ("valid".into(), ValueLiteral::boolean(true)),
            ("mode".into(), mode.value(1).unwrap()),
            (
                "voltage".into(),
                ValueLiteral::new(volts.clone(), [(12.0, 0.0)]).unwrap(),
            ),
        ];
        let admitted = record.value(value.clone()).unwrap();
        assert_eq!(admitted.definition(), record.id());
        assert_eq!(admitted.members()[0].value_type(), &volts);
        assert_eq!(admitted.members()[1].enum_tag(), Some(1));
        assert_eq!(admitted.members()[2].as_bool(), Some(true));
        assert_ne!(
            admitted,
            RecordDef::new(Id::new(), members)
                .unwrap()
                .value(value.clone())
                .unwrap()
        );
        let mut missing = value.clone();
        missing.pop();
        assert!(record.value(missing).is_err());
        let mut duplicate = value.clone();
        duplicate[0] = duplicate[1].clone();
        assert!(record.value(duplicate).is_err());
        let mut foreign = value.clone();
        foreign[0].0 = "unknown".into();
        assert!(record.value(foreign).is_err());
        let mut wrong = value;
        wrong[1].1 = ValueLiteral::boolean(false);
        assert!(record.value(wrong).is_err());
    }
}
