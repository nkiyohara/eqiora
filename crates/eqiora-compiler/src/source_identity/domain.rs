use super::*;

pub(super) fn encode_domain(
    encoder: &mut Encoder,
    declaration: &DomainDecl,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    encoder.field(1, |encoder| {
        encode_name(encoder, declaration.name(), budget)
    })?;
    encoder.field(2, |encoder| match declaration.syntax() {
        DomainSyntax::CartesianBox(bounds) => {
            let parameter_backed = bounds.iter().any(|(lower, upper)| {
                matches!(lower, CartesianCoordinateSyntax::Parameter { .. })
                    || matches!(upper, CartesianCoordinateSyntax::Parameter { .. })
            });
            if parameter_backed {
                encoder.u16(4)?;
                encoder.u32(as_u32(bounds.len(), "Cartesian axis count")?)?;
                for (lower, upper) in bounds {
                    encode_cartesian_coordinate(encoder, lower, budget)?;
                    encode_cartesian_coordinate(encoder, upper, budget)?;
                }
            } else {
                encoder.u16(1)?;
                encoder.u32(as_u32(bounds.len(), "Cartesian axis count")?)?;
                for (lower, upper) in bounds {
                    let (
                        CartesianCoordinateSyntax::Fixed { value: lower, .. },
                        CartesianCoordinateSyntax::Fixed { value: upper, .. },
                    ) = (lower, upper)
                    else {
                        unreachable!("fixed Cartesian identity was checked above");
                    };
                    encoder.f64(*lower)?;
                    encoder.f64(*upper)?;
                }
            }
            Ok(())
        }
        DomainSyntax::Boundary { parent, axis, side } => {
            encoder.u16(2)?;
            encoder.field(1, |encoder| encode_name(encoder, parent, budget))?;
            encoder.field(2, |encoder| {
                encoder.u64(u64::try_from(*axis).map_err(|_| {
                    source_identity_error("boundary axis does not fit canonical u64")
                })?)
            })?;
            encoder.field(3, |encoder| {
                encoder.u8(match side {
                    BoundarySideSyntax::Lower => 1,
                    BoundarySideSyntax::Upper => 2,
                })
            })
        }
        DomainSyntax::ScalarPhysical {
            across_dimension,
            through_dimension,
        } => {
            encoder.u16(3)?;
            encoder.field(1, |encoder| {
                encode_expression(encoder, across_dimension, budget, 1)
            })?;
            encoder.field(2, |encoder| {
                encode_expression(encoder, through_dimension, budget, 1)
            })
        }
        _ => Err(source_identity_error(
            "Domain syntax is newer than source identity v1",
        )),
    })
}

fn encode_cartesian_coordinate(
    encoder: &mut Encoder,
    coordinate: &CartesianCoordinateSyntax,
    budget: &mut Budget,
) -> Result<(), Diagnostic> {
    match coordinate {
        CartesianCoordinateSyntax::Fixed { value, .. } => {
            encoder.u8(1)?;
            encoder.f64(*value)
        }
        CartesianCoordinateSyntax::Parameter { name, .. } => {
            encoder.u8(2)?;
            encode_name(encoder, name, budget)
        }
    }
}
