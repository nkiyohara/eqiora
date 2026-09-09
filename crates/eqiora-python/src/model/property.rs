//! Exact immutable property occurrence metadata for Model inspection.
use super::*;

/// Immutable inspection of one exact package-owned typed property binding.
#[pyclass(
    name = "PropertyBinding",
    module = "eqiora._eqiora",
    frozen,
    skip_from_py_object
)]
#[derive(Debug, Clone)]
pub(crate) struct PyPropertyBinding {
    composition: Option<String>,
    contract: String,
    release: String,
    component: String,
    requirement: String,
    meaning: eqiora::kernel::PropertyMeaning,
    inputs: Vec<String>,
    branch: Option<String>,
    derivatives: String,
    validity: String,
    citation: String,
    license: String,
}

#[pymethods]
impl PyPropertyBinding {
    #[getter]
    fn composition(&self) -> Option<&str> {
        self.composition.as_deref()
    }

    #[getter]
    fn contract(&self) -> &str {
        &self.contract
    }

    #[getter]
    fn release(&self) -> &str {
        &self.release
    }

    #[getter]
    fn component(&self) -> &str {
        &self.component
    }

    #[getter]
    fn requirement(&self) -> &str {
        &self.requirement
    }

    #[getter]
    fn normalized_value(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self.meaning.constant_value() {
            Some(value) => crate::modeling::value_literal::to_python(py, value),
            None => Ok(py.None()),
        }
    }

    #[getter]
    fn value_type(&self) -> crate::modeling::PyValueType {
        crate::modeling::PyValueType {
            value: self
                .meaning
                .value_type()
                .expect("admitted complete property type"),
        }
    }

    #[getter]
    fn inputs(&self) -> Vec<String> {
        self.inputs.clone()
    }
    #[getter]
    fn branch(&self) -> Option<&str> {
        self.branch.as_deref()
    }
    #[getter]
    fn derivatives(&self) -> &str {
        &self.derivatives
    }

    #[getter]
    fn first_partials(&self) -> bool {
        self.derivatives != "value_only"
    }

    #[getter]
    fn validity(&self) -> &str {
        &self.validity
    }

    #[getter]
    fn citation(&self) -> &str {
        &self.citation
    }

    #[getter]
    fn license(&self) -> &str {
        &self.license
    }

    fn __repr__(&self) -> String {
        format!(
            "PropertyBinding(composition={:?}, contract={:?}, release={:?}, component={:?}, requirement={:?}, normalized_value={:?}, validity={:?}, citation={:?}, license={:?})",
            self.composition,
            self.contract,
            self.release,
            self.component,
            self.requirement,
            self.meaning,
            self.validity,
            self.citation,
            self.license,
        )
    }
}

pub(super) fn property_bindings<'a>(
    nodes: impl Iterator<Item = &'a eqiora::kernel::KernelNode>,
) -> Box<[PyPropertyBinding]> {
    nodes
        .filter_map(|node| match node {
            eqiora::kernel::KernelNode::Relation(relation) => Some(relation),
            _ => None,
        })
        .flat_map(|relation| {
            relation
                .expression()
                .properties()
                .iter()
                .map(move |(root, release)| {
                    let (component, requirement) = release
                        .consumer()
                        .map(|(component, requirement)| {
                            (component.to_owned(), requirement.to_owned())
                        })
                        .unwrap_or_else(|| {
                            (
                                relation.id().erase().ulid().to_string(),
                                format!("expression.{}", root.index()),
                            )
                        });
                    PyPropertyBinding {
                        composition: release.composition().map(str::to_owned),
                        contract: release.contract().to_owned(),
                        release: release.release().to_owned(),
                        component,
                        requirement,
                        meaning: release.meaning().clone(),
                        inputs: release.inputs().to_vec(),
                        branch: release.branch().map(str::to_owned),
                        derivatives: release.derivatives().as_str().to_owned(),
                        validity: if release.guarded() {
                            "checked"
                        } else {
                            "unconditional"
                        }
                        .to_owned(),
                        citation: release.citation().to_owned(),
                        license: release.license().to_owned(),
                    }
                })
        })
        .collect()
}
