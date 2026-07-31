use crate::cartesian::CartesianCoordinateSyntax;

pub(super) fn format_cartesian_coordinate(
    coordinate: &CartesianCoordinateSyntax,
    output: &mut String,
) {
    match coordinate {
        CartesianCoordinateSyntax::Fixed { value, .. } => {
            output.push_str(&super::format_number(*value));
        }
        CartesianCoordinateSyntax::Parameter { name, .. } => output.push_str(name),
    }
}
