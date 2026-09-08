//! Scalar SSA vocabulary shared by typed and numerical execution.
use super::SymbolSlot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ValueId(pub(super) u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Instruction {
    Constant(eqiora_core::DynQuantity),
    TypedConstant(u32),
    Array {
        start: u32,
        len: u32,
    },
    PureOperator {
        definition: u32,
        start: u32,
        len: u32,
    },
    Index(ValueId, u32),
    Quotient(ValueId, ValueId),
    Remainder(ValueId, ValueId),
    ToReal(ValueId),
    ToInteger(ValueId),
    Ordinal(ValueId),
    Min(ValueId, ValueId),
    Max(ValueId, ValueId),
    Compare(eqiora_schema::kernel::ComparisonOp, ValueId, ValueId),
    Select {
        condition: ValueId,
        then_value: ValueId,
        else_value: ValueId,
    },
    Require {
        condition: ValueId,
        value: ValueId,
    },
    Sqrt(ValueId),
    Not(ValueId),
    And(ValueId, ValueId),
    Or(ValueId, ValueId),
    Read(SymbolSlot),
    Neg(ValueId),
    Add(ValueId, ValueId),
    Sub(ValueId, ValueId),
    Mul(ValueId, ValueId),
    Div(ValueId, ValueId),
    PowI(ValueId, i32),
}
