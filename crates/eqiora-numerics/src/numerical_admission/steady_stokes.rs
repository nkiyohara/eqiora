use super::*;

impl CommonSteadyStokesPlan {
    fn reauthenticate_portable_realization(&self) -> Result<(), Diagnostic> {
        require_portable_realization(&self.portable, self.resolved.portable_graph()?)
    }

    /// Effective mixed Galerkin Formulation and its automatic-selection audit.
    #[must_use]
    pub fn formulation(&self) -> CommonFormulationDescription {
        CommonFormulationDescription::mixed(
            self.binding.formulation_correspondence(),
            self.formulation_selection,
            match self.formulation_selection {
                FormulationSelectionMode::Automatic => {
                    "eqiora.formulation.auto.mixed-galerkin-for-mini-p1/v1"
                }
                FormulationSelectionMode::Exact => {
                    "eqiora.formulation.exact.mixed-galerkin-admitted/v1"
                }
            },
        )
    }

    pub(super) fn from_admission(
        model: &ModelEnvelope,
        admission: NativeNumericalAdmission,
        formulation_selection: FormulationSelectionMode,
        scaling: ResolvedIncompressibleScaling2d,
    ) -> Result<Self, Diagnostic> {
        let model_reference = model.artifact_reference()?;
        let model_id = model_reference.model().ulid().to_string();
        let binding = admission.stokes_binding()?;
        let (resolved, portable, velocity_space, pressure_space) =
            admission.resolve_stokes(&binding)?;
        let (geometry_digest, mesh_digest, correspondence_digest, production_digest) =
            resource_digests(admission.resources())?;
        let mut velocity_field_id = None;
        let mut pressure_field_id = None;
        for field in resolved.plan().spatial().field_spaces() {
            match field.space().family() {
                SpaceFamily::SimplexP1Bubble => {
                    velocity_field_id = Some(field.field().ulid().to_string());
                }
                SpaceFamily::ContinuousLagrange { order } if order == std::num::NonZeroU16::MIN => {
                    pressure_field_id = Some(field.field().ulid().to_string());
                }
                _ => {}
            }
        }
        let (velocity_field_id, pressure_field_id) = velocity_field_id
            .zip(pressure_field_id)
            .ok_or_else(|| invalid("steady-Stokes Plan omitted its MINI/P1 Field identities"))?;
        let realization_digest = hex_bytes(&portable.digest()?);
        let scaling_provenance_digest = scaling.receipt().provenance_digest();
        let mut identity_bytes = Vec::new();
        for value in [
            admission.model_digest(),
            geometry_digest.as_str(),
            mesh_digest.as_str(),
            correspondence_digest.as_str(),
            production_digest.as_str(),
            realization_digest.as_str(),
            admission.policy_identity(),
            scaling_provenance_digest.as_str(),
        ] {
            push_framed(&mut identity_bytes, value.as_bytes());
        }
        push_framed(&mut identity_bytes, formulation_selection.identity());
        let identity = hex_bytes(&Sha256::digest(
            [
                b"eqiora.common-steady-stokes-plan/v1\0".as_slice(),
                identity_bytes.as_slice(),
            ]
            .concat(),
        ));
        Ok(Self {
            admission,
            binding,
            resolved,
            portable,
            formulation_selection,
            scaling,
            realization_digest,
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
        })
    }

    /// Execute solely from the state retained by this Plan.
    pub fn run(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<crate::fluid::SteadyStokesMiniSolution2d, Diagnostic> {
        self.reauthenticate_portable_realization()?;
        self.admission.revalidate()?;
        if backend.provider() != self.admission.linear.provider
            || backend.capabilities() != self.admission.linear.capabilities
        {
            return Err(invalid(
                "steady-Stokes execution backend differs from the admitted provider or capabilities",
            ));
        }
        let solution = solve_resolved_steady_stokes_geometry_mini_2d(
            &self.admission.program,
            &self.resolved,
            &self.binding,
            backend,
        )?;
        if solution.scales() != self.scaling.scales() {
            return Err(invalid(
                "steady-Stokes execution changed the Plan-owned effective scaling",
            ));
        }
        Ok(solution)
    }

    /// Project scientific observations only after reauthenticating this Plan's roles.
    pub(super) fn observe(
        &self,
        solution: &crate::fluid::SteadyStokesMiniSolution2d,
    ) -> Result<CommonSteadyStokesObservation, Diagnostic> {
        self.admission.revalidate()?;
        if solution.velocity_field().ulid().to_string() != self.velocity_field_id
            || solution.pressure_field().ulid().to_string() != self.pressure_field_id
            || solution.scales() != self.scaling.scales()
        {
            return Err(invalid(
                "steady-Stokes observation crossed a different Plan or effective scaling",
            ));
        }
        for role in ["cylinder", "inlet", "outlet", "walls"] {
            if self.binding.entities(role)?.is_empty() {
                return Err(invalid(format!(
                    "steady-Stokes observation role `{role}` has no authenticated support"
                )));
            }
        }
        let bounds = self
            .admission
            .resources
            .geometry()
            .circular_hole_bounds()
            .copied()
            .ok_or_else(|| invalid("steady-Stokes observation requires circular-hole Geometry"))?;
        let pressure = solution.pressure().vertex_values();
        let pressure_minimum = pressure
            .iter()
            .copied()
            .min_by(f64::total_cmp)
            .ok_or_else(|| invalid("steady-Stokes observation has no pressure values"))?;
        let pressure_maximum = pressure
            .iter()
            .copied()
            .max_by(f64::total_cmp)
            .ok_or_else(|| invalid("steady-Stokes observation has no pressure values"))?;
        let cylinder_force_on_fluid =
            solution
                .named_boundary_reaction("cylinder")
                .ok_or_else(|| {
                    invalid("steady-Stokes solution omitted authenticated cylinder reaction")
                })?;
        let inlet_flux = solution
            .named_boundary_flux("inlet")
            .ok_or_else(|| invalid("steady-Stokes solution omitted authenticated inlet flux"))?;
        let outlet_flux = solution
            .named_boundary_flux("outlet")
            .ok_or_else(|| invalid("steady-Stokes solution omitted authenticated outlet flux"))?;
        let constrained_reaction = solution.boundary_reaction();
        let integrated_body_force = solution.integrated_body_force();
        let integrated_boundary_traction = solution.integrated_boundary_traction();
        let momentum_closure = std::array::from_fn(|component| {
            constrained_reaction[component]
                + integrated_body_force[component]
                + integrated_boundary_traction[component]
        });
        let net_flux = inlet_flux + outlet_flux;
        let continuity_residual_norm = solution.dimensionless_solution().continuity_residual_norm();
        if bounds
            .iter()
            .flatten()
            .copied()
            .chain([
                pressure_minimum,
                pressure_maximum,
                inlet_flux,
                outlet_flux,
                net_flux,
            ])
            .chain(cylinder_force_on_fluid)
            .chain(constrained_reaction)
            .chain(integrated_body_force)
            .chain(integrated_boundary_traction)
            .chain(momentum_closure)
            .chain([continuity_residual_norm])
            .any(|value| !value.is_finite())
        {
            return Err(invalid(
                "steady-Stokes observation contains a non-finite value",
            ));
        }
        Ok(CommonSteadyStokesObservation {
            pressure_minimum,
            pressure_maximum,
            exact_bounds: bounds,
            cylinder_force_on_fluid,
            inlet_flux,
            outlet_flux,
            net_flux,
            constrained_reaction,
            integrated_body_force,
            integrated_boundary_traction,
            momentum_closure,
            solve: solution.dimensionless_solution().solve_report().clone(),
            continuity_residual_norm,
        })
    }

    /// Execute and authenticate observations without exposing a re-pairing seam.
    pub fn run_observed(
        &self,
        backend: &dyn LinearSolverBackend,
    ) -> Result<CommonSteadyStokesRunOutput, Diagnostic> {
        let solution = self.run(backend)?;
        let observation = self.observe(&solution)?;
        Ok(CommonSteadyStokesRunOutput {
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
    pub fn realization_digest(&self) -> &str {
        &self.realization_digest
    }
    /// Canonical portable numerical realization owned by this Plan.
    #[must_use]
    pub const fn portable_realization(&self) -> &PortableRealizationGraph {
        &self.portable
    }
    #[must_use]
    pub const fn scaling_receipt(&self) -> &IncompressibleScalingReceipt2d {
        self.scaling.receipt()
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
    pub const fn velocity_space(&self) -> Space {
        self.velocity_space
    }
    #[must_use]
    pub const fn pressure_space(&self) -> Space {
        self.pressure_space
    }
    #[must_use]
    pub const fn scales(&self) -> IncompressibleFlowScaleProfile2d {
        self.scaling.scales()
    }
    #[must_use]
    pub const fn linear(&self) -> SolverPlan {
        self.admission.linear.solver
    }
}
