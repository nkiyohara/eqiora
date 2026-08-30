use super::*;
use crate::form_compiler::derive_candidate;

fn describe_primal(
    kind: FormulationKind,
    boundary_treatment: &'static str,
    rule_ids: [&'static str; 4],
    requested: FormulationSelectionMode,
) -> CommonFormulationDescription {
    CommonFormulationDescription {
        requested,
        kind,
        boundary_treatment,
        rule_ids: rule_ids.into(),
        selection_reason_codes: Box::new([match requested {
            FormulationSelectionMode::Automatic => {
                "eqiora.formulation.auto.primal-galerkin-for-q1/v1"
            }
            FormulationSelectionMode::Exact => {
                "eqiora.formulation.exact.primal-galerkin-admitted/v1"
            }
            FormulationSelectionMode::Authored => {
                "eqiora.formulation.authored.primal-galerkin-admitted/v1"
            }
        }]),
        requested_source_identity: None,
    }
}

pub(super) fn resolve_common_scalar_portable(
    admission: &NativeNumericalAdmission,
    lowered: &ScalarEllipticCartesianModel,
    mesh: &CartesianMeshEnvelopeV1,
    cells: [usize; 2],
) -> Result<PortableRealizationGraph, Diagnostic> {
    let cells = cells
        .map(|count| NonZeroUsize::new(count).expect("validated Cartesian cells are non-zero"));
    let (method, space, quadrature) = match admission.spatial {
        NativeSpatialPolicy::ScalarQ1 => (
            DiscretizationMethod::ContinuousGalerkin,
            Space::continuous_lagrange(std::num::NonZeroU16::MIN),
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
        ),
        NativeSpatialPolicy::ScalarTpfa => (
            DiscretizationMethod::CellCenteredFiniteVolume,
            Space::cell_constant(),
            QuadraturePolicy::CellCentroid,
        ),
        NativeSpatialPolicy::ElasticityQ1
        | NativeSpatialPolicy::StokesMiniP1(_)
        | NativeSpatialPolicy::TransientMiniP1(_)
        | NativeSpatialPolicy::TransientCellCentered(_) => {
            return Err(invalid(
                "common scalar portable graph received a non-scalar spatial policy",
            ));
        }
    };
    let solver = admission.linear.solver;
    admission.linear.capabilities.require_problem(
        solver,
        ScalarType::F64,
        LinearOperatorProperties::SymmetricPositiveDefinite,
    )?;
    PortableRealizationGraph::linear_single_field(
        RealizationLineage::explicit(
            admission.program.model(),
            SemanticRevision::new(admission.program.revision().0),
            RealizationRevision::new(COMMON_SCALAR_REALIZATION_REVISION),
        ),
        lowered.domain_id(),
        lowered.field_id(),
        space,
        Discretization::new(
            method,
            MeshPolicy::SuppliedCartesian {
                artifact: mesh.artifact_reference()?,
                cells,
            },
            quadrature,
        ),
        LinearOperatorProperties::SymmetricPositiveDefinite,
        ScalarType::F64,
        VectorLayoutKind::Replicated,
        solver,
        Target::HostCpu {
            threads: admission.linear.workers,
        },
        ExecutionSchedule::Offline,
    )
}

impl CommonScalarPlan {
    fn reauthenticate_portable_realization(&self) -> Result<(), Diagnostic> {
        let NativeMeshResources::Cartesian { mesh, .. } = &self.admission.resources else {
            return Err(invalid(
                "common scalar Plan lost its exact Cartesian Mesh materialization",
            ));
        };
        let RecognizedNativeModel::Scalar(lowered) = &self.admission.recognized else {
            return Err(invalid(
                "common scalar Plan lost its recognized mathematical materialization",
            ));
        };
        if let Some(authored) = &self.authored_formulation {
            let derived = lowered.compiled_form.as_ref().ok_or_else(|| {
                invalid("authored scalar-primal Plan lost its effective derived Formulation")
            })?;
            crate::form_compiler::admit_authored_scalar_primal_form(
                authored,
                &self.admission.program,
                derived,
            )?;
        }
        require_portable_realization(
            &self.portable,
            resolve_common_scalar_portable(&self.admission, lowered, mesh, self.cells)?,
        )
    }

    pub(super) fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        formulation_selection: Option<FormulationSelectionMode>,
        authored_formulation: Option<&AuthoredFormulationProjection>,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let NativeMeshResources::Cartesian {
            geometry,
            mesh,
            correspondence,
            production,
        } = &admission.resources
        else {
            return Err(invalid(
                "scalar Q1/TPFA common Plan requires an authenticated Cartesian Mesh",
            ));
        };
        let geometry_digest = hex_bytes(&geometry.digest_bytes());
        let mesh_digest = mesh.digest()?.to_string();
        let correspondence_digest = correspondence.digest()?.to_string();
        let production_digest = production.digest()?.to_string();
        let cells = [
            mesh.mesh()
                .axis_cell_count(0)
                .ok_or_else(|| invalid("common Plan Mesh omitted x-axis cells"))?,
            mesh.mesh()
                .axis_cell_count(1)
                .ok_or_else(|| invalid("common Plan Mesh omitted y-axis cells"))?,
        ];
        let RecognizedNativeModel::Scalar(lowered) = &admission.recognized else {
            return Err(invalid(
                "common scalar Plan admitted non-scalar mathematics",
            ));
        };
        let mut accepted_authored_formulation = None;
        let formulation = match formulation_selection {
            None => None,
            Some(selection) => {
                match derive_candidate(&admission.program, lowered.domain_id().erase())? {
                    Some(derived) => {
                        if let Some(projection) = authored_formulation {
                            crate::form_compiler::admit_authored_scalar_primal_form(
                                projection,
                                &admission.program,
                                &derived,
                            )?;
                            accepted_authored_formulation = Some(projection.clone());
                        }
                        let (kind, boundary_treatment, rule_ids) =
                            derived.formulation_description();
                        let mut description = describe_primal(
                            kind,
                            boundary_treatment,
                            rule_ids,
                            if authored_formulation.is_some() {
                                FormulationSelectionMode::Authored
                            } else {
                                selection
                            },
                        );
                        description.requested_source_identity = accepted_authored_formulation
                            .as_ref()
                            .map(|form| form.source_identity().to_owned());
                        Some(description)
                    }
                    None if authored_formulation.is_some() => {
                        return Err(invalid(
                            "authored scalar Q1 primal Formulation requires the admitted complete homogeneous-essential boundary class",
                        ));
                    }
                    None if selection == FormulationSelectionMode::Automatic => None,
                    None => {
                        return Err(invalid(
                            "exact scalar Q1 primal Formulation requires the admitted complete homogeneous-essential boundary class",
                        ));
                    }
                }
            }
        };
        let portable = resolve_common_scalar_portable(&admission, lowered, mesh, cells)?;
        let realization_digest = hex_bytes(&portable.digest()?);
        let field = lowered.field_id();
        let field_id = field.ulid().to_string();
        let field_dimension = match admission.program.node(field.erase()) {
            Some(KernelNode::Field(definition)) => definition.dimension(),
            _ => {
                return Err(invalid(
                    "common scalar admission lost its exact semantic Field definition",
                ));
            }
        };
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            realization_digest.as_str(),
            admission.policy_identity(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        push_framed(
            &mut identity_bytes,
            formulation
                .as_ref()
                .map(|description| description.requested().identity())
                .unwrap_or(b"no-proof-carrying-formulation"),
        );
        if let Some(authored) = &accepted_authored_formulation {
            push_framed(&mut identity_bytes, authored.source_identity().as_bytes());
            push_framed(&mut identity_bytes, authored.canonical_bytes());
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-scalar-plan/v2\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            portable,
            formulation,
            authored_formulation: accepted_authored_formulation,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            realization_digest,
            field,
            field_id,
            field_dimension,
            cells,
        })
    }

    pub(crate) fn run(&self) -> Result<ResolvedScalarEllipticCartesianSolution, Diagnostic> {
        self.reauthenticate_portable_realization()?;
        self.admission.execute_scalar(&REFERENCE_LINEAR_SOLVER)
    }

    /// Effective primal Galerkin Formulation for Q1, when one is admitted.
    #[must_use]
    pub fn formulation(&self) -> Option<CommonFormulationDescription> {
        self.formulation.clone()
    }

    pub(crate) fn authored_formulation_bytes(&self) -> Option<&[u8]> {
        self.authored_formulation
            .as_ref()
            .map(AuthoredFormulationProjection::canonical_bytes)
    }

    /// Execute solely from retained Plan state and publish one complete Result.
    pub fn run_result(&self) -> Result<crate::CommonResult, Diagnostic> {
        crate::CommonResult::accept_scalar(self.clone(), 0.0, self.run()?)
    }

    /// Accept one selected Parameter point through this Plan's exact supplied Mesh and policies.
    ///
    /// `values=None` selects the Model's canonical values. Otherwise only the ordered selected
    /// Parameter values vary; Model structure and every numerical resource remain Plan-owned.
    pub fn differentiate(
        &self,
        selected: &[eqiora_core::Id<eqiora_core::entity::kinds::Parameter>],
        values: Option<&[f64]>,
    ) -> Result<CommonScalarDifferentiationPoint, Diagnostic> {
        self.reauthenticate_portable_realization()?;
        self.admission.revalidate()?;
        let RecognizedNativeModel::Scalar(template) = &self.admission.recognized else {
            return Err(invalid(
                "common scalar Plan lost its recognized mathematics",
            ));
        };
        let selected_values = selected
            .iter()
            .map(|field| {
                template
                    .parameter_fields()
                    .iter()
                    .position(|candidate| candidate == field)
                    .map(|index| template.parameter_values()[index])
                    .ok_or_else(|| {
                        invalid(
                            "selected differentiable Parameter is frozen or absent from this Plan",
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let bound = template
            .bind_selected_parameters(selected, values.unwrap_or(selected_values.as_slice()))?;
        let NativeMeshResources::Cartesian { mesh, .. } = &self.admission.resources else {
            return Err(invalid(
                "common scalar differentiation requires exact Cartesian resources",
            ));
        };
        let mesh = mesh.mesh();
        let source = |coordinates: &[f64]| bound.source().evaluate(coordinates).unwrap_or(f64::NAN);
        let coefficient = |coordinates: &[f64]| {
            bound
                .coefficient_expression()
                .evaluate(coordinates)
                .unwrap_or(f64::NAN)
        };
        let boundary = |axis: usize, side: BoundarySide, coordinates: &[f64]| {
            let condition = bound
                .boundary(axis, side)
                .expect("lowered Cartesian model owns every side");
            let value = condition.value().evaluate(coordinates).unwrap_or(f64::NAN);
            match condition {
                ScalarEllipticCartesianBoundary::Essential(_) => {
                    CartesianBoundaryValue::Essential(value)
                }
                ScalarEllipticCartesianBoundary::Natural(_) => {
                    CartesianBoundaryValue::Natural(value)
                }
            }
        };
        let solver = self.admission.linear.solver;
        let target = Target::HostCpu {
            threads: NonZeroUsize::MIN,
        };
        let finalized = match self.admission.spatial {
            NativeSpatialPolicy::ScalarQ1 => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                let assembly = finalize_scalar_elliptic_cartesian_fem(
                    mesh,
                    &coefficient,
                    &source,
                    &boundary,
                    &quadrature,
                    &REFERENCE_ASSEMBLY_BACKEND,
                    None,
                )?;
                FinalizedScalarEllipticCartesianProblem::finite_element(
                    self.portable.clone(),
                    solver,
                    VectorLayoutKind::Replicated,
                    target,
                    assembly,
                )?
            }
            NativeSpatialPolicy::ScalarTpfa => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                let assembly = finalize_scalar_elliptic_cartesian_fvm(
                    mesh,
                    &coefficient,
                    &source,
                    &boundary,
                    &cell,
                    &facet,
                    &REFERENCE_ASSEMBLY_BACKEND,
                )?;
                FinalizedScalarEllipticCartesianProblem::finite_volume(
                    self.portable.clone(),
                    solver,
                    VectorLayoutKind::Replicated,
                    target,
                    assembly,
                )?
            }
            NativeSpatialPolicy::ElasticityQ1
            | NativeSpatialPolicy::StokesMiniP1(_)
            | NativeSpatialPolicy::TransientMiniP1(_)
            | NativeSpatialPolicy::TransientCellCentered(_) => {
                return Err(invalid(
                    "common scalar differentiation received a non-scalar spatial policy",
                ));
            }
        };
        let executor = HostExecutorDescriptor::new(
            self.admission.linear.provider,
            self.admission.linear.execution,
            self.admission.linear.workers,
            self.admission.linear.capabilities.clone(),
        );
        let binding = DeploymentBinding::bind_host(&self.portable, executor)?;
        let admitted = AdmittedExecution::admit_host_linear(
            &self.portable,
            finalized.canonical_csr_system_view(),
            binding,
        )?;
        let produced = REFERENCE_LINEAR_SOLVER.solve(&finalized.linear_problem()?, solver)?;
        let accepted = admitted.accept(produced)?;
        let (solution, receipt) = accepted.into_parts();
        let solution = finalized.finish(solution)?;
        let coordinates = selected
            .iter()
            .copied()
            .map(SpatialDesignCoordinate::ModelParameter)
            .collect::<Vec<_>>();
        let (relation, output) = match &solution {
            ResolvedScalarEllipticCartesianSolution::FiniteElement(solution) => {
                let quadrature = QuadratureRule::tensor_product_gauss_legendre(2, 2)?;
                (
                    linearize_scalar_elliptic_cartesian_fem(
                        &bound,
                        mesh,
                        solution,
                        &quadrature,
                        &coordinates,
                    )?,
                    linearize_scalar_elliptic_cartesian_fem_output(
                        &bound,
                        mesh,
                        solution,
                        &coordinates,
                    )?,
                )
            }
            ResolvedScalarEllipticCartesianSolution::FiniteVolume(solution) => {
                let cell = QuadratureRule::tensor_product_gauss_legendre(2, 1)?;
                let facet = QuadratureRule::gauss_legendre(1)?;
                (
                    linearize_scalar_elliptic_cartesian_fvm(
                        &bound,
                        mesh,
                        solution,
                        &cell,
                        &facet,
                        &coordinates,
                    )?,
                    linearize_scalar_elliptic_cartesian_fvm_output(
                        &bound,
                        mesh,
                        solution,
                        &coordinates,
                    )?,
                )
            }
        };
        if relation.state_jacobian().agreement_fingerprint() != receipt.operator() {
            return Err(invalid(
                "common Plan solve receipt differs from its differentiated state system",
            ));
        }
        Ok(CommonScalarDifferentiationPoint {
            relation,
            output,
            receipt,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    #[must_use]
    pub const fn model_revision(&self) -> u64 {
        self.model_revision
    }

    #[must_use]
    pub fn model_digest(&self) -> &str {
        self.admission.model_digest()
    }

    /// Exact canonical Model artifact selected by this Plan.
    pub fn model_reference(&self) -> Result<eqiora_artifact::ModelArtifactReference, Diagnostic> {
        self.admission.model.artifact_reference()
    }

    #[must_use]
    pub fn geometry_digest(&self) -> &str {
        &self.geometry_digest
    }

    #[must_use]
    pub fn mesh_digest(&self) -> &str {
        &self.mesh_digest
    }

    #[must_use]
    pub fn correspondence_digest(&self) -> &str {
        &self.correspondence_digest
    }

    #[must_use]
    pub fn production_digest(&self) -> &str {
        &self.production_digest
    }

    #[must_use]
    pub fn realization_digest(&self) -> &str {
        &self.realization_digest
    }

    /// Canonical portable numerical realization owned by this Plan.
    #[must_use]
    pub const fn portable_realization(&self) -> &PortableRealizationGraph {
        &self.portable
    }

    #[must_use]
    pub fn field_id(&self) -> &str {
        &self.field_id
    }

    /// Exact scalar Field represented by this Plan.
    #[must_use]
    pub const fn field(&self) -> eqiora_core::Id<eqiora_core::entity::kinds::Field> {
        self.field
    }

    /// Coherent-SI dimension of the exact scalar Field represented by this Plan.
    #[must_use]
    pub const fn field_dimension(&self) -> DimExponents {
        self.field_dimension
    }

    #[must_use]
    pub const fn cells(&self) -> [usize; 2] {
        self.cells
    }

    #[must_use]
    pub fn spatial(&self) -> CommonSpatialPolicy {
        match self.admission.spatial {
            NativeSpatialPolicy::ScalarQ1 => CommonSpatialPolicy::Q1,
            NativeSpatialPolicy::ScalarTpfa => CommonSpatialPolicy::CellCenteredTpfa,
            NativeSpatialPolicy::ElasticityQ1 => {
                unreachable!("common scalar Plan cannot own elasticity policy")
            }
            NativeSpatialPolicy::StokesMiniP1(_) => {
                unreachable!("common scalar Plan cannot own Stokes policy")
            }
            NativeSpatialPolicy::TransientMiniP1(_)
            | NativeSpatialPolicy::TransientCellCentered(_) => {
                unreachable!("common scalar Plan cannot own transient-flow policy")
            }
        }
    }

    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}
