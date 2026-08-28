use super::*;

impl CommonTransientFlowPlan {
    pub(super) fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        scaling: ResolvedIncompressibleScaling2d,
        temporal: CommonBackwardEuler,
        nonlinear: NonlinearSolvePlan,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let model_id = model_reference.model().ulid().to_string();
        let RecognizedNativeModel::Transient(transient) = &admission.recognized else {
            return Err(invalid(
                "transient Plan admission omitted recognized transient Model meaning",
            ));
        };
        let velocity_field_id = transient.velocity().ulid().to_string();
        let pressure_field_id = transient.pressure().ulid().to_string();
        let solver = admission.linear.solver;
        let (resolved, velocity_space, pressure_space, gauge) =
            match (admission.spatial, &admission.resources) {
                (
                    NativeSpatialPolicy::TransientMiniP1(scales),
                    NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
                ) => {
                    let plan = transient_navier_stokes_mini_plan_2d(
                        transient,
                        mesh.artifact_reference()?,
                        scales,
                        temporal.step(),
                        nonlinear,
                        solver,
                    )?;
                    let capabilities = transient_realization_capabilities(
                        DiscretizationMethod::ContinuousGalerkin,
                        MeshKind::ImportedAffineSimplicial,
                        solver,
                        &admission.linear.capabilities,
                    )?;
                    let resolved = resolve_transient_fieldwise(
                        &TransientFieldwiseRealizationRequest::explicit(
                            admission.program.model(),
                            SemanticRevision::new(admission.program.revision().0),
                            RealizationRevision::new(TRANSIENT_REALIZATION_REVISION),
                            plan,
                        ),
                        transient_navier_stokes_fieldwise_requirements_2d(transient),
                        &capabilities,
                    )?;
                    let gauge = if resolved
                        .plan()
                        .fieldwise()
                        .spatial()
                        .constraints()
                        .is_empty()
                    {
                        CommonPressureGauge2d::BoundaryTraction
                    } else {
                        CommonPressureGauge2d::ZeroIntegral
                    };
                    (
                        CommonTransientResolvedSpatial::MiniP1(resolved),
                        Space::simplex_p1_bubble(),
                        Space::continuous_lagrange(std::num::NonZeroU16::MIN),
                        gauge,
                    )
                }
                (
                    NativeSpatialPolicy::TransientCellCentered(scales),
                    NativeMeshResources::Cartesian { mesh, .. },
                ) => {
                    let artifact = mesh.artifact_reference()?;
                    let cells =
                        [
                            NonZeroUsize::new(mesh.mesh().axis_cell_count(0).ok_or_else(|| {
                                invalid("supplied Cartesian mesh omitted x cells")
                            })?)
                            .ok_or_else(|| invalid("supplied Cartesian x cell count is zero"))?,
                            NonZeroUsize::new(mesh.mesh().axis_cell_count(1).ok_or_else(|| {
                                invalid("supplied Cartesian mesh omitted y cells")
                            })?)
                            .ok_or_else(|| invalid("supplied Cartesian y cell count is zero"))?,
                        ];
                    let plan = transient_navier_stokes_cell_centered_plan_2d(
                        transient,
                        MeshPolicy::SuppliedCartesian { artifact, cells },
                        scales,
                        temporal.step(),
                        nonlinear,
                        solver,
                    )?;
                    let capabilities = transient_realization_capabilities(
                        DiscretizationMethod::CellCenteredFiniteVolume,
                        MeshKind::SuppliedCartesian,
                        solver,
                        &admission.linear.capabilities,
                    )?;
                    let resolved = resolve_transient_cell_centered_incompressible_flow(
                        &TransientCellCenteredIncompressibleFlowRealizationRequest::explicit(
                            admission.program.model(),
                            SemanticRevision::new(admission.program.revision().0),
                            RealizationRevision::new(TRANSIENT_REALIZATION_REVISION),
                            plan,
                        ),
                        transient_navier_stokes_cell_centered_requirements_2d(transient),
                        &TransientCellCenteredIncompressibleFlowCapabilities::new(capabilities),
                    )?;
                    (
                        CommonTransientResolvedSpatial::CellCentered(resolved),
                        Space::cell_constant(),
                        Space::cell_constant(),
                        CommonPressureGauge2d::ZeroIntegral,
                    )
                }
                _ => {
                    return Err(invalid(
                        "transient spatial policy and exact caller Mesh are cross-wired",
                    ));
                }
            };
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(&admission.resources)?;
        let receipt_digest = scaling.receipt().provenance_digest();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            model_id.as_str(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            admission.policy_identity(),
            receipt_digest.as_str(),
            velocity_field_id.as_str(),
            pressure_field_id.as_str(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        identity_bytes.extend_from_slice(&model_reference.semantic_revision().get().to_be_bytes());
        identity_bytes.extend_from_slice(&TRANSIENT_REALIZATION_REVISION.to_be_bytes());
        identity_bytes.extend_from_slice(&COMMON_TRANSIENT_RESOLVER_EPOCH.to_be_bytes());
        match &admission.resources {
            NativeMeshResources::AffineTriangleSimplicial { mesh, .. } => {
                push_framed(&mut identity_bytes, b"imported-affine-simplicial");
                push_framed(
                    &mut identity_bytes,
                    mesh.artifact_reference()?.sha256().as_slice(),
                );
            }
            NativeMeshResources::Cartesian { mesh, .. } => {
                push_framed(&mut identity_bytes, b"supplied-cartesian");
                push_framed(
                    &mut identity_bytes,
                    mesh.artifact_reference()?.sha256().as_slice(),
                );
                for axis in 0..2 {
                    let count = mesh.mesh().axis_cell_count(axis).ok_or_else(|| {
                        invalid("supplied Cartesian transient Mesh omitted an axis")
                    })?;
                    identity_bytes.extend_from_slice(&count.to_be_bytes());
                }
            }
            NativeMeshResources::AdjacentPartitionSimplicial { .. }
            | NativeMeshResources::GmshSimplicial { .. } => {
                return Err(invalid(
                    "transient common Plan requires the exact caller affine-triangle or supplied-Cartesian envelope",
                ));
            }
        }
        push_framed(&mut identity_bytes, space_identity(velocity_space));
        push_framed(&mut identity_bytes, space_identity(pressure_space));
        push_framed(
            &mut identity_bytes,
            match gauge {
                CommonPressureGauge2d::ZeroIntegral => b"zero-integral",
                CommonPressureGauge2d::BoundaryTraction => b"boundary-traction",
            },
        );
        for discriminant in [
            b"f64".as_slice(),
            b"replicated".as_slice(),
            b"general-operator".as_slice(),
            b"host-cpu".as_slice(),
            b"host-serial".as_slice(),
            b"offline".as_slice(),
        ] {
            push_framed(&mut identity_bytes, discriminant);
        }
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-transient-flow-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            resolved,
            scaling,
            temporal,
            nonlinear,
            identity,
            model_id,
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            velocity_field_id,
            pressure_field_id,
            velocity_space,
            pressure_space,
            gauge,
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
    pub const fn scaling_receipt(&self) -> &IncompressibleScalingReceipt2d {
        self.scaling.receipt()
    }
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scaling.scales()
    }
    #[must_use]
    pub const fn temporal(&self) -> CommonBackwardEuler {
        self.temporal
    }
    #[must_use]
    pub const fn nonlinear(&self) -> NonlinearSolvePlan {
        self.nonlinear
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
    #[must_use]
    pub fn velocity_field_id(&self) -> &str {
        &self.velocity_field_id
    }
    #[must_use]
    pub fn pressure_field_id(&self) -> &str {
        &self.pressure_field_id
    }
    #[must_use]
    pub fn domain_id(&self) -> String {
        match &self.admission.recognized {
            RecognizedNativeModel::Transient(model) => model.domain().ulid().to_string(),
            _ => unreachable!("common transient Plan retains transient Model meaning"),
        }
    }
    #[must_use]
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
    }
    #[must_use]
    pub const fn gauge(&self) -> CommonPressureGauge2d {
        self.gauge
    }
    #[must_use]
    pub const fn spatial(&self) -> CommonSpatialPolicy {
        match self.resolved {
            CommonTransientResolvedSpatial::MiniP1(_) => CommonSpatialPolicy::MiniP1,
            CommonTransientResolvedSpatial::CellCentered(_) => CommonSpatialPolicy::CellCentered,
        }
    }

    /// Identity of the complete restartable state space, excluding solve and Run controls.
    #[must_use]
    pub fn state_space_identity(&self) -> String {
        let mut bytes = Vec::new();
        for value in [
            self.model_digest(),
            self.geometry_digest(),
            self.mesh_digest(),
            self.correspondence_digest(),
            self.production_digest(),
            self.velocity_field_id(),
            self.pressure_field_id(),
            "f64",
            "replicated",
        ] {
            push_framed(&mut bytes, value.as_bytes());
        }
        push_framed(&mut bytes, space_identity(self.velocity_space()));
        push_framed(&mut bytes, space_identity(self.pressure_space()));
        push_framed(
            &mut bytes,
            match self.gauge() {
                CommonPressureGauge2d::ZeroIntegral => b"zero-integral",
                CommonPressureGauge2d::BoundaryTraction => b"boundary-traction",
            },
        );
        push_framed(
            &mut bytes,
            match self.spatial() {
                CommonSpatialPolicy::MiniP1 => b"mini-p1/backward-euler/no-extra-history/v1",
                CommonSpatialPolicy::CellCentered => {
                    b"cell-centered/backward-euler/bdf1-previous-accepted-face-volume-flux/v1"
                }
                _ => unreachable!("closed transient spatial policy"),
            },
        );
        hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-state-space/v1\0".as_slice(),
                bytes.as_slice(),
            ]
            .concat(),
        ))
    }

    /// Construct the sole explicit homogeneous-zero bootstrap for this transient Plan.
    pub fn zero_state(&self, time_s: f64) -> Result<CommonState, Diagnostic> {
        self.admission.revalidate()?;
        let RecognizedNativeModel::Transient(model) = &self.admission.recognized else {
            return Err(invalid("State.zero requires a transient Plan"));
        };
        crate::canonical_stokes::require_complete_zero_trace(model)?;
        let time = DynQuantity::new(time_s, TIME);
        let kind = match (&self.resolved, &self.admission.resources) {
            (
                CommonTransientResolvedSpatial::MiniP1(_),
                NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
            ) => {
                let mesh_data = mesh.mesh().clone();
                let vertex_count = mesh_data.vertices().len();
                let cell_count = mesh_data.entity_count(2).ok_or_else(|| {
                    invalid("affine-triangle transient Mesh omitted two-dimensional cells")
                })?;
                let reference = match self.gauge {
                    CommonPressureGauge2d::ZeroIntegral => {
                        SteadyStokesPressureReference2d::ZeroIntegral { multiplier: 0.0 }
                    }
                    CommonPressureGauge2d::BoundaryTraction => {
                        SteadyStokesPressureReference2d::BoundaryTraction
                    }
                };
                CommonStateKind::MiniP1(Box::new(TransientNavierStokesInitialState2d::new(
                    model,
                    time,
                    mesh.artifact_reference()?,
                    SimplicialMiniVelocityField2d::new(
                        mesh_data.clone(),
                        vec![[0.0; 2]; vertex_count],
                        vec![[0.0; 2]; cell_count],
                    )?,
                    SimplicialP1Field::new(mesh_data, vec![0.0; vertex_count])?,
                    reference,
                )?))
            }
            (
                CommonTransientResolvedSpatial::CellCentered(_),
                NativeMeshResources::Cartesian { mesh, .. },
            ) => {
                let mesh_data = mesh.mesh().clone();
                let cell_count = mesh_data.entity_count(2).ok_or_else(|| {
                    invalid("Cartesian transient Mesh omitted two-dimensional cells")
                })?;
                let facet_count =
                    crate::cartesian_fvm_geometry::cartesian_fvm_geometry_2d(&mesh_data)?
                        .1
                        .len();
                CommonStateKind::CellCentered(Box::new(
                    CellCenteredNavierStokesInitialState2d::new(
                        model,
                        time,
                        CellCenteredVelocityField2d::new(
                            mesh_data.clone(),
                            vec![[0.0; 2]; cell_count],
                        )?,
                        CellCenteredPressureField2d::new(mesh_data, vec![0.0; cell_count])?,
                        0.0,
                        vec![0.0; facet_count],
                    )?,
                ))
            }
            _ => {
                return Err(invalid(
                    "transient Plan lost its exact caller Mesh envelope",
                ));
            }
        };
        CommonState::new(
            self.state_space_identity(),
            time_s,
            Arc::new(self.admission.model.clone()),
            Arc::new(self.admission.resources.clone()),
            kind,
        )
    }

    /// Advance exactly one accepted Backward-Euler step from a compatible complete State.
    pub fn advance_one(
        &self,
        state: &CommonState,
        backend: &dyn LinearSolverBackend,
    ) -> Result<CommonState, Diagnostic> {
        self.admission.revalidate()?;
        if state.state_space_identity != self.state_space_identity() {
            return Err(invalid(
                "State belongs to a different exact common state space",
            ));
        }
        if backend.provider() != self.admission.linear.provider
            || backend.capabilities() != self.admission.linear.capabilities
        {
            return Err(invalid(
                "transient execution backend differs from the admitted provider or capabilities",
            ));
        }
        let run = TransientNavierStokesRun2d::new(NonZeroStepCount::new(NonZeroUsize::MIN));
        let next_kind = match (&self.resolved, &self.admission.resources, &state.kind) {
            (
                CommonTransientResolvedSpatial::MiniP1(resolved),
                NativeMeshResources::AffineTriangleSimplicial { mesh, .. },
                CommonStateKind::MiniP1(initial),
            ) => {
                let trajectory = advance_resolved_transient_navier_stokes_mini_2d(
                    &self.admission.program,
                    resolved,
                    mesh,
                    initial.as_ref().clone(),
                    run,
                    backend,
                )?;
                let accepted = trajectory
                    .states()
                    .last()
                    .ok_or_else(|| invalid("MINI transient step returned no accepted State"))?;
                CommonStateKind::MiniP1(Box::new(mini_initial_from_resolved(self, mesh, accepted)?))
            }
            (
                CommonTransientResolvedSpatial::CellCentered(resolved),
                NativeMeshResources::Cartesian { mesh, .. },
                CommonStateKind::CellCentered(initial),
            ) => {
                let trajectory = advance_resolved_transient_navier_stokes_cell_centered_2d(
                    &self.admission.program,
                    resolved,
                    mesh,
                    initial.as_ref().clone(),
                    run,
                    backend,
                )?;
                let accepted = trajectory.states().last().ok_or_else(|| {
                    invalid("cell-centered transient step returned no accepted State")
                })?;
                CommonStateKind::CellCentered(Box::new(cell_centered_initial_from_resolved(
                    self, accepted,
                )?))
            }
            _ => {
                return Err(invalid(
                    "State method history is incompatible with this Plan",
                ));
            }
        };
        let next_time = state.time_s + self.temporal.step().value();
        CommonState::new(
            self.state_space_identity(),
            next_time,
            Arc::clone(&state.model),
            Arc::clone(&state.resources),
            next_kind,
        )
    }
}

pub(super) fn mini_initial_from_resolved(
    plan: &CommonTransientFlowPlan,
    mesh: &SimplicialMeshEnvelopeV1,
    state: &ResolvedTransientNavierStokesState2d,
) -> Result<TransientNavierStokesInitialState2d, Diagnostic> {
    let RecognizedNativeModel::Transient(model) = &plan.admission.recognized else {
        return Err(invalid("transient Plan lost recognized Model meaning"));
    };
    TransientNavierStokesInitialState2d::new(
        model,
        state.time(),
        mesh.artifact_reference()?,
        state.velocity().clone(),
        state.pressure().clone(),
        state.pressure_reference(),
    )
}

pub(super) fn cell_centered_initial_from_resolved(
    plan: &CommonTransientFlowPlan,
    state: &ResolvedCellCenteredNavierStokesState2d,
) -> Result<CellCenteredNavierStokesInitialState2d, Diagnostic> {
    let RecognizedNativeModel::Transient(model) = &plan.admission.recognized else {
        return Err(invalid("transient Plan lost recognized Model meaning"));
    };
    CellCenteredNavierStokesInitialState2d::new(
        model,
        state.time(),
        state.velocity().clone(),
        state.pressure().clone(),
        state.gauge_multiplier(),
        state.previous_face_volume_fluxes().to_vec(),
    )
}

pub(super) fn transient_realization_capabilities(
    method: DiscretizationMethod,
    mesh: MeshKind,
    solver: SolverPlan,
    backend: &SolverCapabilities,
) -> Result<RealizationCapabilities, Diagnostic> {
    backend.require_problem(solver, ScalarType::F64, LinearOperatorProperties::General)?;
    RealizationCapabilities::cartesian_product(
        [method],
        [(
            mesh,
            SpatialDimensionSupport::exact(NonZeroUsize::new(2).expect("two")),
        )],
        [VectorLayoutKind::Replicated],
        SolverCapabilities::exact([SolverCapability {
            algorithm: solver.algorithm(),
            operator_properties: LinearOperatorProperties::General,
            preconditioner: solver.preconditioner(),
            reduction: solver.reduction(),
            scalar_type: ScalarType::F64,
        }])?,
        TargetCapabilities::none().with_host_cpu(NonZeroUsize::MIN),
    )
}
