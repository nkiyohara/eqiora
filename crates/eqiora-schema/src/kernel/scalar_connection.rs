//! Pure compatibility rules for the current scalar Port families.
//!
//! This module deliberately describes only the scalar kernel contract. Future
//! field-valued physical interfaces can share the same connection algebra
//! without being forced into this closed payload. Source spans, graph paths,
//! and diagnostic prose remain responsibilities of each consuming layer.

use eqiora_core::DimExponents;

use super::SignalDirection;

/// Connection semantics understood by the scalar compatibility contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarConnectionKind {
    /// One causal output drives one or more inputs.
    Signal,
    /// Acausal members form one conserving connection set.
    Conserving,
}

/// Compatibility-relevant type of one scalar Port.
///
/// `I` is the exact nominal identity of a scalar physical connector or Domain.
/// Matching dimensions never substitute for matching that identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ScalarPortContract<I> {
    /// Causal scalar signal.
    Signal {
        /// Direction relative to the owning relation network.
        direction: SignalDirection,
        /// Physical dimension of the carried value.
        dimension: DimExponents,
    },
    /// Structural-only conserving marker.
    ConservingMarker {
        /// Physical dimension of the marker value.
        dimension: DimExponents,
    },
    /// Scalar physical across/through pair with exact nominal identity.
    ScalarPhysical {
        /// Connector or Domain identity defining the physical type.
        nominal: I,
    },
}

/// One structured scalar-connection compatibility failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScalarConnectionViolation {
    /// A connection has fewer than two members.
    TooFewPorts {
        /// Number of supplied Ports.
        found: usize,
    },
    /// A signal connection does not contain exactly one output and otherwise
    /// only inputs.
    SignalDirections {
        /// Number of output Ports.
        outputs: usize,
        /// Number of input Ports.
        inputs: usize,
        /// Total number of Ports, including incompatible kinds.
        total: usize,
    },
    /// Signal Port dimensions are not all equal.
    SignalDimensionMismatch,
    /// A conserving connection mixes scalar Port families.
    MixedConservingFamilies,
    /// Conserving-marker dimensions are not all equal.
    MarkerDimensionMismatch,
    /// Scalar physical Ports do not have one exact nominal identity.
    PhysicalNominalMismatch,
}

/// Validate the current scalar Port compatibility algebra.
///
/// The check is independent of syntax ordering and graph storage. Signal
/// causality follows Port direction: exactly one output and one or more inputs
/// must share one dimension. Conserving markers share one dimension, while
/// scalar physical Ports share one exact nominal identity.
///
/// # Errors
/// Returns the first structural incompatibility in a stable rule order.
pub fn validate_scalar_connection<I: Eq>(
    kind: ScalarConnectionKind,
    ports: &[ScalarPortContract<I>],
) -> Result<(), ScalarConnectionViolation> {
    if ports.len() < 2 {
        return Err(ScalarConnectionViolation::TooFewPorts { found: ports.len() });
    }
    match kind {
        ScalarConnectionKind::Signal => validate_signal(ports),
        ScalarConnectionKind::Conserving => validate_conserving(ports),
    }
}

fn validate_signal<I>(ports: &[ScalarPortContract<I>]) -> Result<(), ScalarConnectionViolation> {
    let mut outputs = 0;
    let mut inputs = 0;
    let mut first_dimension = None;
    let mut dimensions_match = true;
    for port in ports {
        let ScalarPortContract::Signal {
            direction,
            dimension,
        } = port
        else {
            continue;
        };
        match direction {
            SignalDirection::Input => inputs += 1,
            SignalDirection::Output => outputs += 1,
        }
        if let Some(first) = first_dimension {
            dimensions_match &= first == *dimension;
        } else {
            first_dimension = Some(*dimension);
        }
    }
    if outputs != 1 || inputs + outputs != ports.len() {
        return Err(ScalarConnectionViolation::SignalDirections {
            outputs,
            inputs,
            total: ports.len(),
        });
    }
    if !dimensions_match {
        return Err(ScalarConnectionViolation::SignalDimensionMismatch);
    }
    Ok(())
}

fn validate_conserving<I: Eq>(
    ports: &[ScalarPortContract<I>],
) -> Result<(), ScalarConnectionViolation> {
    match &ports[0] {
        ScalarPortContract::ConservingMarker { dimension } => {
            let mut dimensions_match = true;
            for port in ports {
                let ScalarPortContract::ConservingMarker {
                    dimension: candidate,
                } = port
                else {
                    return Err(ScalarConnectionViolation::MixedConservingFamilies);
                };
                dimensions_match &= candidate == dimension;
            }
            if dimensions_match {
                Ok(())
            } else {
                Err(ScalarConnectionViolation::MarkerDimensionMismatch)
            }
        }
        ScalarPortContract::ScalarPhysical { nominal } => {
            let mut nominal_matches = true;
            for port in ports {
                let ScalarPortContract::ScalarPhysical { nominal: candidate } = port else {
                    return Err(ScalarConnectionViolation::MixedConservingFamilies);
                };
                nominal_matches &= candidate == nominal;
            }
            if nominal_matches {
                Ok(())
            } else {
                Err(ScalarConnectionViolation::PhysicalNominalMismatch)
            }
        }
        ScalarPortContract::Signal { .. } => {
            Err(ScalarConnectionViolation::MixedConservingFamilies)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UNIT: DimExponents = DimExponents::DIMENSIONLESS;
    const LENGTH: DimExponents =
        DimExponents::from_integers([0, 1, 0, 0, 0, 0, 0]).expect("bounded dimension");

    fn signal(direction: SignalDirection, dimension: DimExponents) -> ScalarPortContract<u8> {
        ScalarPortContract::Signal {
            direction,
            dimension,
        }
    }

    #[test]
    fn signal_is_one_output_plus_dimension_matched_inputs() {
        let valid = [
            signal(SignalDirection::Input, LENGTH),
            signal(SignalDirection::Output, LENGTH),
            signal(SignalDirection::Input, LENGTH),
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Signal, &valid),
            Ok(())
        );

        let two_outputs = [
            signal(SignalDirection::Output, UNIT),
            signal(SignalDirection::Output, UNIT),
        ];
        assert!(matches!(
            validate_scalar_connection(ScalarConnectionKind::Signal, &two_outputs),
            Err(ScalarConnectionViolation::SignalDirections { outputs: 2, .. })
        ));

        let mismatched = [
            signal(SignalDirection::Output, UNIT),
            signal(SignalDirection::Input, LENGTH),
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Signal, &mismatched),
            Err(ScalarConnectionViolation::SignalDimensionMismatch)
        );
    }

    #[test]
    fn conserving_markers_match_dimension_but_physical_ports_match_identity() {
        let marker_mismatch = [
            ScalarPortContract::<u8>::ConservingMarker { dimension: UNIT },
            ScalarPortContract::ConservingMarker { dimension: LENGTH },
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Conserving, &marker_mismatch),
            Err(ScalarConnectionViolation::MarkerDimensionMismatch)
        );

        let nominal_mismatch = [
            ScalarPortContract::ScalarPhysical { nominal: 1_u8 },
            ScalarPortContract::ScalarPhysical { nominal: 2_u8 },
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Conserving, &nominal_mismatch),
            Err(ScalarConnectionViolation::PhysicalNominalMismatch)
        );

        let exact_nominal = [
            ScalarPortContract::ScalarPhysical { nominal: 7_u8 },
            ScalarPortContract::ScalarPhysical { nominal: 7_u8 },
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Conserving, &exact_nominal),
            Ok(())
        );
    }

    #[test]
    fn connection_families_never_coerce() {
        let mixed = [
            ScalarPortContract::ConservingMarker { dimension: UNIT },
            ScalarPortContract::ScalarPhysical { nominal: 1_u8 },
        ];
        assert_eq!(
            validate_scalar_connection(ScalarConnectionKind::Conserving, &mixed),
            Err(ScalarConnectionViolation::MixedConservingFamilies)
        );
        assert_eq!(
            validate_scalar_connection(
                ScalarConnectionKind::Signal,
                &[signal(SignalDirection::Output, UNIT)]
            ),
            Err(ScalarConnectionViolation::TooFewPorts { found: 1 })
        );
    }
}
