//! Mathematical contractions shared by scalar and mixed region integration.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pairing {
    Value,
    Gradient,
    SymmetricGradient,
    Divergence,
    TestDivergenceTrialValue,
    TestValueTrialDivergence,
}

#[derive(Clone, Copy)]
pub(super) struct Basis<'a> {
    pub value: f64,
    pub gradient: &'a [f64],
    pub component: usize,
}

impl Pairing {
    pub(super) fn entry(self, test: Basis<'_>, trial: Basis<'_>) -> f64 {
        match self {
            Self::Value if test.component == trial.component => test.value * trial.value,
            Self::Gradient if test.component == trial.component => {
                crate::affine_fem::dot(test.gradient, trial.gradient)
            }
            Self::Value | Self::Gradient => 0.0,
            Self::SymmetricGradient => {
                0.5 * crate::continuum_kinematics::symmetric_gradient_bilinear_entry(
                    test.gradient,
                    test.component,
                    trial.gradient,
                    trial.component,
                )
            }
            Self::Divergence => test.gradient[test.component] * trial.gradient[trial.component],
            Self::TestDivergenceTrialValue => test.gradient[test.component] * trial.value,
            Self::TestValueTrialDivergence => test.value * trial.gradient[trial.component],
        }
    }
}
