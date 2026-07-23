/// Coordinate-frame meaning of mathematical value components.
///
/// Version one intentionally admits only invariant values and components in
/// the model-global Cartesian spatial frame. Arbitrary local frames and frame
/// transforms require an explicit future contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ValueFrame {
    /// Components are unchanged by a Cartesian spatial frame change.
    Invariant,
    /// Components are expressed in the model-global Cartesian spatial frame.
    SpatialCartesian,
}
