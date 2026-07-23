use eqiora_core::Diagnostic;
use eqiora_core::diagnostic::codes;

use crate::{BackendId, ExecutionId};

/// One exact library version compiled into an execution provider.
///
/// Runtime-discovered native libraries remain adapter-specific observations.
/// This value describes only a dependency release declared by the compiled
/// provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProviderLibrary {
    name: &'static str,
    version: &'static str,
}

impl ProviderLibrary {
    /// Construct one compile-time library-version observation.
    #[must_use]
    pub const fn new(name: &'static str, version: &'static str) -> Self {
        Self { name, version }
    }

    /// Stable library identity.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.name
    }

    /// Exact version compiled into the provider.
    #[must_use]
    pub const fn version(self) -> &'static str {
        self.version
    }
}

macro_rules! provider_descriptor {
    ($name:ident, $identity:ty, $kind:literal) => {
        #[doc = concat!(
            "Stable ",
            $kind,
            " identity paired with its declared implementation release and dependency inventory."
        )]
        ///
        /// The stable ID does not absorb a version: different provider
        /// releases may execute the same mathematical Realization, while Run
        /// evidence retains the exact release that executed.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            id: $identity,
            implementation_version: &'static str,
            libraries: &'static [ProviderLibrary],
        }

        impl $name {
            /// Construct one static provider release descriptor.
            ///
            /// Validation occurs at capability binding and report acceptance,
            /// where a structured diagnostic can be returned without
            /// preventing use in constants.
            #[must_use]
            pub const fn new(
                id: $identity,
                implementation_version: &'static str,
                libraries: &'static [ProviderLibrary],
            ) -> Self {
                Self {
                    id,
                    implementation_version,
                    libraries,
                }
            }

            /// Stable provider ID, independent of its implementation version.
            #[must_use]
            pub const fn id(self) -> $identity {
                self.id
            }

            /// Exact Eqiora provider release compiled into the run.
            #[must_use]
            pub const fn implementation_version(self) -> &'static str {
                self.implementation_version
            }

            /// Sorted, unique dependency releases declared by the provider.
            #[must_use]
            pub const fn libraries(self) -> &'static [ProviderLibrary] {
                self.libraries
            }

            /// Validate the descriptor before admission or acceptance.
            ///
            /// # Errors
            /// Returns `EQ0807` for invalid identity/version text or an
            /// unsorted, duplicate, or invalid library inventory.
            pub fn validate(self) -> Result<(), Diagnostic> {
                validate_provider(
                    concat!($kind, " provider"),
                    self.id.as_str(),
                    self.implementation_version,
                    self.libraries,
                )
            }
        }
    };
}

provider_descriptor!(SolverProvider, BackendId, "solver");
provider_descriptor!(ExecutionProvider, ExecutionId, "execution");

fn validate_provider(
    kind: &str,
    id: &str,
    implementation_version: &str,
    libraries: &[ProviderLibrary],
) -> Result<(), Diagnostic> {
    validate_text(&format!("{kind} ID"), id)?;
    validate_text(
        &format!("{kind} implementation version"),
        implementation_version,
    )?;
    let mut previous = None;
    for library in libraries {
        validate_library_name(library.name)?;
        validate_text("provider library version", library.version)?;
        if previous.is_some_and(|name| name >= library.name) {
            return Err(invalid_provider(
                "provider libraries must be sorted by unique ascending name",
            ));
        }
        previous = Some(library.name);
    }
    Ok(())
}

fn validate_library_name(value: &str) -> Result<(), Diagnostic> {
    if value.is_empty()
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-' | b'_')
        })
    {
        return Err(invalid_provider(
            "provider library names must be non-empty lowercase dotted/kebab/snake ASCII",
        ));
    }
    Ok(())
}

fn validate_text(label: &str, value: &str) -> Result<(), Diagnostic> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(invalid_provider(format!(
            "{label} must be non-empty text without surrounding whitespace or control characters"
        )));
    }
    Ok(())
}

fn invalid_provider(message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(codes::INVALID_REALIZATION, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &[ProviderLibrary] = &[];
    const LIBRARIES: &[ProviderLibrary] = &[
        ProviderLibrary::new("faer", "0.24.4"),
        ProviderLibrary::new("rayon", "1.12.0"),
    ];
    const INVALID_NAME: &[ProviderLibrary] = &[ProviderLibrary::new("", "1.0.0")];
    const NON_DURABLE_NAME: &[ProviderLibrary] = &[ProviderLibrary::new("Not Durable", "1.0.0")];
    const UNSORTED: &[ProviderLibrary] = &[
        ProviderLibrary::new("rayon", "1.12.0"),
        ProviderLibrary::new("faer", "0.24.4"),
    ];
    const DUPLICATE: &[ProviderLibrary] = &[
        ProviderLibrary::new("rayon", "1.12.0"),
        ProviderLibrary::new("rayon", "1.12.0"),
    ];

    #[test]
    fn provider_descriptors_preserve_stable_ids_and_exact_releases() {
        let provider =
            SolverProvider::new(BackendId::new("eqiora.test.solver"), "0.1.0", LIBRARIES);

        provider.validate().unwrap();
        assert_eq!(provider.id().as_str(), "eqiora.test.solver");
        assert_eq!(provider.implementation_version(), "0.1.0");
        assert_eq!(provider.libraries(), LIBRARIES);
    }

    #[test]
    fn provider_validation_rejects_invalid_text_and_library_order() {
        for provider in [
            ExecutionProvider::new(ExecutionId::new(""), "0.1.0", EMPTY),
            ExecutionProvider::new(ExecutionId::new("eqiora.test"), " ", EMPTY),
            ExecutionProvider::new(ExecutionId::new("eqiora.test"), "0.1.0", INVALID_NAME),
            ExecutionProvider::new(ExecutionId::new("eqiora.test"), "0.1.0", NON_DURABLE_NAME),
            ExecutionProvider::new(ExecutionId::new("eqiora.test"), "0.1.0", UNSORTED),
            ExecutionProvider::new(ExecutionId::new("eqiora.test"), "0.1.0", DUPLICATE),
        ] {
            assert_eq!(
                provider.validate().unwrap_err().code(),
                codes::INVALID_REALIZATION
            );
        }
    }
}
