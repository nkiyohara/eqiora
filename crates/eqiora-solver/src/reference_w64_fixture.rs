use std::sync::atomic::{AtomicUsize, Ordering};

use eqiora_core::Diagnostic;

use crate::{
    FixedOrderInnerProduct, LinearOperator, ReductionPolicy, ReplicatedLinearExecution,
    SERIAL_LINEAR_EXECUTION,
};

pub(super) const HADAMARD_CONDITIONING_INTEGERS: [u64; 64] = [
    1,
    2,
    3,
    4,
    5,
    6,
    8,
    12,
    17,
    24,
    34,
    48,
    68,
    97,
    138,
    197,
    280,
    398,
    565,
    804,
    1_143,
    1_625,
    2_311,
    3_287,
    4_674,
    6_647,
    9_452,
    13_440,
    19_112,
    27_178,
    38_648,
    54_958,
    78_151,
    111_131,
    158_030,
    224_721,
    319_557,
    454_415,
    646_185,
    918_884,
    1_306_667,
    1_858_100,
    2_642_246,
    3_757_313,
    5_342_955,
    7_597_761,
    10_804_128,
    15_363_631,
    21_847_311,
    31_067_200,
    44_178_019,
    62_821_798,
    89_333_529,
    127_033_602,
    180_643_666,
    256_877_971,
    365_284_284,
    519_439_668,
    738_650_910,
    1_050_372_547,
    1_493_645_335,
    2_123_985_813,
    3_020_339_320,
    4_294_967_296,
];

#[derive(Debug)]
pub(super) struct HadamardConditionedSymmetricIndefinite;

impl LinearOperator for HadamardConditionedSymmetricIndefinite {
    fn rows(&self) -> usize {
        HADAMARD_CONDITIONING_INTEGERS.len()
    }

    fn columns(&self) -> usize {
        HADAMARD_CONDITIONING_INTEGERS.len()
    }

    fn apply(&self, input: &[f64], output: &mut [f64]) -> Result<(), Diagnostic> {
        if input.len() != self.columns() || output.len() != self.rows() {
            return Err(super::solve_failed("Hadamard witness shape mismatch"));
        }
        let mut spectral = input.to_vec();
        sylvester_hadamard_transform(&mut spectral);
        for (index, value) in spectral.iter_mut().enumerate() {
            let sign = if index % 2 == 0 { -1.0 } else { 1.0 };
            *value *= sign * (HADAMARD_CONDITIONING_INTEGERS[index] as f64) / (8.0 * 65_536.0);
        }
        sylvester_hadamard_transform(&mut spectral);
        for (output, value) in output.iter_mut().zip(spectral) {
            *output = value / 8.0;
        }
        Ok(())
    }
}

fn sylvester_hadamard_transform(values: &mut [f64]) {
    let mut width = 1;
    while width < values.len() {
        for start in (0..values.len()).step_by(2 * width) {
            for offset in 0..width {
                let left = values[start + offset];
                let right = values[start + width + offset];
                values[start + offset] = left + right;
                values[start + width + offset] = left - right;
            }
        }
        width *= 2;
    }
}

pub(super) fn right_hand_side() -> Vec<f64> {
    let mut right_hand_side = HADAMARD_CONDITIONING_INTEGERS
        .iter()
        .enumerate()
        .map(|(index, magnitude)| {
            let sign = if index % 2 == 0 { -1.0 } else { 1.0 };
            sign * (*magnitude as f64) / 65_536.0
        })
        .collect::<Vec<_>>();
    sylvester_hadamard_transform(&mut right_hand_side);
    for value in &mut right_hand_side {
        *value /= 8.0;
    }
    right_hand_side
}

#[derive(Debug, Default)]
pub(super) struct RecordingExecution {
    inner_products: AtomicUsize,
}

impl RecordingExecution {
    pub(super) fn inner_products(&self) -> usize {
        self.inner_products.load(Ordering::Relaxed)
    }
}

impl ReplicatedLinearExecution for RecordingExecution {
    fn provider(&self) -> crate::ExecutionProvider {
        SERIAL_LINEAR_EXECUTION.provider()
    }

    fn report(&self) -> crate::ExecutionReport {
        SERIAL_LINEAR_EXECUTION.report()
    }

    fn require_reduction(&self, policy: ReductionPolicy) -> Result<(), Diagnostic> {
        SERIAL_LINEAR_EXECUTION.require_reduction(policy)
    }

    fn apply(
        &self,
        operator: &dyn LinearOperator,
        input: &[f64],
        output: &mut [f64],
    ) -> Result<(), Diagnostic> {
        SERIAL_LINEAR_EXECUTION.apply(operator, input, output)
    }

    fn inner_product(&self, action: FixedOrderInnerProduct<'_>) -> Result<f64, Diagnostic> {
        self.inner_products.fetch_add(1, Ordering::Relaxed);
        SERIAL_LINEAR_EXECUTION.inner_product(action)
    }
}
