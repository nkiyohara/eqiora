//! Method-neutral fixed-step count.

/// Non-zero number of requested accepted fixed steps.
///
/// This execution control is shared by transient realizations; it does not
/// belong to any one fluid, solid, or coupling method.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonZeroStepCount(std::num::NonZeroUsize);

impl NonZeroStepCount {
    /// Exact accepted-step count.
    #[must_use]
    pub const fn new(value: std::num::NonZeroUsize) -> Self {
        Self(value)
    }

    /// Requested number of accepted fixed steps.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0.get()
    }
}
