use super::*;

fn resolve_common_elasticity_portable(
    admission: &NativeNumericalAdmission,
    lowered: &IsotropicElasticityCartesianModel2d,
    mesh: &CartesianMeshEnvelopeV1,
    cells: [usize; 2],
) -> Result<PortableRealizationGraph, Diagnostic> {
    if admission.spatial != NativeSpatialPolicy::ElasticityQ1 {
        return Err(invalid(
            "common elasticity portable graph received a non-elasticity spatial policy",
        ));
    }
    let domain = lowered
        .domain()
        .downcast::<eqiora_core::entity::kinds::Domain>()
        .ok_or_else(|| invalid("elasticity Domain lost its semantic kind"))?;
    let displacement = lowered
        .displacement()
        .downcast::<eqiora_core::entity::kinds::Field>()
        .ok_or_else(|| invalid("elasticity displacement lost its semantic Field kind"))?;
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
            RealizationRevision::new(COMMON_ELASTICITY_REALIZATION_REVISION),
        ),
        domain,
        displacement,
        Space::continuous_lagrange(std::num::NonZeroU16::MIN),
        Discretization::new(
            DiscretizationMethod::ContinuousGalerkin,
            MeshPolicy::SuppliedCartesian {
                artifact: mesh.artifact_reference()?,
                cells: cells.map(|count| {
                    NonZeroUsize::new(count).expect("validated Cartesian cells are non-zero")
                }),
            },
            QuadraturePolicy::GaussLegendre {
                points_per_axis: NonZeroUsize::new(2).expect("two is non-zero"),
            },
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

impl CommonElasticityPlan {
    fn reauthenticate_portable_realization(&self) -> Result<(), Diagnostic> {
        let NativeMeshResources::Cartesian { mesh, .. } = &self.admission.resources else {
            return Err(invalid(
                "common elasticity Plan lost its exact Cartesian Mesh materialization",
            ));
        };
        let RecognizedNativeModel::Elasticity(lowered) = &self.admission.recognized else {
            return Err(invalid(
                "common elasticity Plan lost its recognized mathematical materialization",
            ));
        };
        require_portable_realization(
            &self.portable,
            resolve_common_elasticity_portable(&self.admission, lowered, mesh, self.cells)?,
        )
    }

    pub(super) fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let NativeMeshResources::Cartesian { mesh, .. } = &admission.resources else {
            return Err(invalid(
                "linear-elasticity common Plan requires an authenticated Cartesian Mesh",
            ));
        };
        let cells = [
            mesh.mesh()
                .axis_cell_count(0)
                .ok_or_else(|| invalid("elasticity Plan Mesh omitted x-axis cells"))?,
            mesh.mesh()
                .axis_cell_count(1)
                .ok_or_else(|| invalid("elasticity Plan Mesh omitted y-axis cells"))?,
        ];
        let RecognizedNativeModel::Elasticity(lowered) = &admission.recognized else {
            return Err(invalid(
                "common elasticity Plan omitted recognized elasticity meaning",
            ));
        };
        let portable = resolve_common_elasticity_portable(&admission, lowered, mesh, cells)?;
        let realization_digest = hex_bytes(&portable.digest()?);
        let displacement_field_id = lowered.displacement().ulid().to_string();
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(admission.resources())?;
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
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-linear-elasticity-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            portable,
            identity,
            model_id: model_reference.model().ulid().to_string(),
            model_revision: model_reference.semantic_revision().get(),
            geometry_digest,
            mesh_digest,
            correspondence_digest,
            production_digest,
            realization_digest,
            displacement_field_id,
            cells,
        })
    }

    pub fn run(&self) -> Result<CartesianLinearElasticity2dSolution, Diagnostic> {
        self.reauthenticate_portable_realization()?;
        self.admission.execute_elasticity(&REFERENCE_LINEAR_SOLVER)
    }

    /// Project scientific observations through this exact admitted Plan.
    pub(super) fn observe(
        &self,
        solution: &CartesianLinearElasticity2dSolution,
    ) -> Result<CommonElasticityObservation, Diagnostic> {
        self.admission.revalidate()?;
        let bounds = self
            .admission
            .resources
            .geometry()
            .planar_rectangle_bounds()
            .copied()
            .ok_or_else(|| {
                invalid("linear-elasticity observation requires rectangular Geometry")
            })?;
        let constrained_reaction = solution.boundary_reaction();
        let integrated_body_force = solution.integrated_body_force();
        if bounds
            .iter()
            .flatten()
            .copied()
            .chain(constrained_reaction)
            .chain(integrated_body_force)
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "linear-elasticity observation contains a non-finite value",
            ));
        }
        let assembly = solution.assembly_report();
        Ok(CommonElasticityObservation {
            constrained_reaction,
            integrated_body_force,
            assembly_packets: assembly.packet_count(),
            assembly_targets: assembly.target_count(),
            solve: solution.solve_report().clone(),
            exact_bounds: bounds,
        })
    }

    /// Execute and authenticate observations without exposing a re-pairing seam.
    pub fn run_observed(&self) -> Result<CommonElasticityRunOutput, Diagnostic> {
        let solution = self.run()?;
        let observation = self.observe(&solution)?;
        Ok(CommonElasticityRunOutput {
            plan_identity: self.identity.clone(),
            solution,
            observation,
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
    pub fn realization_digest(&self) -> &str {
        &self.realization_digest
    }
    /// Canonical portable numerical realization owned by this Plan.
    #[must_use]
    pub const fn portable_realization(&self) -> &PortableRealizationGraph {
        &self.portable
    }
    #[must_use]
    pub fn displacement_field_id(&self) -> &str {
        &self.displacement_field_id
    }
    #[must_use]
    pub const fn cells(&self) -> [usize; 2] {
        self.cells
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}
