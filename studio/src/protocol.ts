import { z } from "zod";

export const BRIDGE_PROTOCOL = "eqiora.studio.bridge/v5" as const;
export const MAX_REQUESTED_STEPS = 5_000_000;
export const MAX_SPATIAL_ENTITY_COUNT = 250_000;
export const MAX_PROJECTION_NODE_COUNT = 100_000;
export const MAX_PROJECTION_EDGE_COUNT = 400_000;

export const sourceSpanSchema = z
  .object({
    file: z.string().max(4_096),
    start: z.number().int().nonnegative(),
    end: z.number().int().nonnegative(),
  })
  .refine((span) => span.end >= span.start, {
    message: "Source span end must not precede its start.",
  });

export type SourceSpan = z.infer<typeof sourceSpanSchema>;

export const diagnosticSchema = z.object({
  source: z.enum(["control", "kernel", "studio"]),
  severity: z.enum(["error", "warning", "note"]),
  code: z.string().min(1).max(32),
  message: z.string().min(1).max(16_384),
  graphPath: z.union([z.string().max(2_048), z.array(z.string())]).nullable(),
  span: sourceSpanSchema.nullable(),
  patch: z
    .object({ summary: z.string().min(1) })
    .strict()
    .nullable()
    .optional(),
});

export type StudioDiagnostic = z.infer<typeof diagnosticSchema>;

export const projectionNodeKindSchema = z.enum([
  "domain",
  "representation",
  "field",
  "parameter",
  "port",
  "relation",
  "activation",
  "connection",
  "clock-domain",
]);

export type ProjectionNodeKind = z.infer<typeof projectionNodeKindSchema>;

export const projectionNodeSchema = z.object({
  id: z.string().min(1).max(128),
  name: z.string().min(1).max(512),
  kind: projectionNodeKindSchema,
  summary: z.string().min(1).max(2_048),
  dimension: z.string().max(128).nullable(),
  value: z.number().finite().nullable(),
});

export type ProjectionNode = z.infer<typeof projectionNodeSchema>;

export const projectionEdgeSchema = z.object({
  id: z.string().min(1).max(384),
  source: z.string().min(1).max(128),
  target: z.string().min(1).max(128),
  kind: z.string().min(1).max(64),
  label: z.string().min(1).max(128),
});

export type ProjectionEdge = z.infer<typeof projectionEdgeSchema>;

export const documentProjectionSchema = z.object({
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  revision: z.number().int().nonnegative(),
  modelId: z.string().min(1).max(128),
  nodes: z.array(projectionNodeSchema).max(MAX_PROJECTION_NODE_COUNT),
  edges: z.array(projectionEdgeSchema).max(MAX_PROJECTION_EDGE_COUNT),
  workflows: z.object({
    scalarElliptic: z
      .object({
        spatialDimension: z.number().int().min(1).max(3),
        scalarType: z.literal("f64"),
        vectorLayout: z.literal("replicated"),
        maximumHostWorkers: z.number().int().positive().max(64),
        workerBudgetSource: z.literal("studio-session-budget"),
      })
      .nullable(),
  }),
});

export type DocumentProjection = z.infer<typeof documentProjectionSchema>;

const runControlFields = {
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  endTime: z.number().finite().positive(),
  maxStep: z.number().finite().positive(),
} as const;

function withinStepLimit(request: { readonly endTime: number; readonly maxStep: number }) {
  return Math.ceil(request.endTime / request.maxStep) <= MAX_REQUESTED_STEPS;
}

export const runPreviewRequestSchema = z.object(runControlFields).refine(withinStepLimit, {
  message: `Run request exceeds the ${MAX_REQUESTED_STEPS.toLocaleString("en-US")}-step bridge limit.`,
  path: ["maxStep"],
});

export type RunPreviewRequest = z.infer<typeof runPreviewRequestSchema>;

export const runRequestSchema = z
  .object({
    ...runControlFields,
    runId: z.string().uuid(),
    planKey: z.string().min(1).max(256),
  })
  .refine(withinStepLimit, {
    message: `Run request exceeds the ${MAX_REQUESTED_STEPS.toLocaleString("en-US")}-step bridge limit.`,
    path: ["maxStep"],
  });

export type RunRequest = z.infer<typeof runRequestSchema>;

export const cancelRunRequestSchema = z.object({
  protocol: z.literal(BRIDGE_PROTOCOL),
  runId: z.string().uuid(),
});

export type CancelRunRequest = z.infer<typeof cancelRunRequestSchema>;

const valueEditControlFields = {
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  targetId: z.string().min(1).max(128),
  value: z.number().finite(),
} as const;

export const valueEditPreviewRequestSchema = z.object(valueEditControlFields);
export type ValueEditPreviewRequest = z.infer<typeof valueEditPreviewRequestSchema>;

export const valueEditCommitRequestSchema = z.object({
  ...valueEditControlFields,
  planKey: z.string().min(1).max(256),
});
export type ValueEditCommitRequest = z.infer<typeof valueEditCommitRequestSchema>;

const spatialRealizationFields = {
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  realizationRevision: z.number().int().nonnegative(),
  method: z.enum(["finite-element", "finite-volume"]),
  cellsPerAxis: z.number().int().positive(),
  workers: z.number().int().positive().max(64),
} as const;

export const spatialRealizationPreviewRequestSchema = z.object(spatialRealizationFields);
export type SpatialRealizationPreviewRequest = z.infer<
  typeof spatialRealizationPreviewRequestSchema
>;

export const spatialRealizationRunRequestSchema = z.object({
  ...spatialRealizationFields,
  runId: z.string().uuid(),
  planKey: z.string().regex(/^[0-9a-f]{64}$/),
});
export type SpatialRealizationRunRequest = z.infer<typeof spatialRealizationRunRequestSchema>;

export const quantitySchema = z.object({
  value: z.number().finite(),
  dimension: z.string().max(128),
});

export const valueEditPlanSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    key: z.string().min(1).max(256),
    baseDigest: z.string().min(16).max(128),
    baseRevision: z.number().int().nonnegative(),
    targetId: z.string().min(1).max(128),
    before: quantitySchema,
    after: quantitySchema,
    transactionDigest: z.string().min(64).max(128),
  })
  .refine((plan) => plan.before.dimension === plan.after.dimension, {
    message: "Value edit cannot change physical dimension.",
    path: ["after", "dimension"],
  })
  .refine((plan) => plan.before.value !== plan.after.value, {
    message: "Value edit must change canonical content.",
    path: ["after", "value"],
  });
export type ValueEditPlan = z.infer<typeof valueEditPlanSchema>;

export const valueEditEvidenceSchema = z.object({
  plan: valueEditPlanSchema,
  resultDigest: z.string().min(16).max(128),
  resultRevision: z.number().int().nonnegative(),
});
export type ValueEditEvidence = z.infer<typeof valueEditEvidenceSchema>;

export const valueEditResultSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    document: documentProjectionSchema,
    evidence: valueEditEvidenceSchema,
  })
  .refine(
    (result) =>
      result.document.digest === result.evidence.resultDigest &&
      result.document.revision === result.evidence.resultRevision,
    {
      message: "Edited document and transaction evidence lineage differ.",
      path: ["evidence"],
    },
  );
export type ValueEditResult = z.infer<typeof valueEditResultSchema>;

const spatialDiscretizationSchema = z
  .object({
    method: z.enum(["finite-element", "finite-volume"]),
    space: z.enum(["continuous-lagrange", "cell-constant"]),
    order: z.number().int().positive().nullable(),
    mesh: z.literal("generated-cartesian"),
    cellsPerAxis: z.number().int().positive(),
    cellCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
    quadrature: z.enum(["gauss-legendre", "cell-centroid"]),
    pointsPerAxis: z.number().int().positive().nullable(),
    fieldValueCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
  })
  .superRefine((value, context) => {
    const coherentFem =
      value.method === "finite-element" &&
      value.space === "continuous-lagrange" &&
      value.order === 1 &&
      value.quadrature === "gauss-legendre" &&
      value.pointsPerAxis === 2;
    const coherentFvm =
      value.method === "finite-volume" &&
      value.space === "cell-constant" &&
      value.order === null &&
      value.quadrature === "cell-centroid" &&
      value.pointsPerAxis === null;
    if (!coherentFem && !coherentFvm) {
      context.addIssue({
        code: "custom",
        message: "Spatial method, space, and quadrature are incoherent.",
        path: ["method"],
      });
    }
  });

export const spatialRunPlanSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    key: z.string().regex(/^[0-9a-f]{64}$/),
    modelDigest: z.string().min(16).max(128),
    realizationRevision: z.number().int().nonnegative(),
    requirements: z.object({
      spatialDimension: z.number().int().min(1).max(3),
      scalarType: z.literal("f64"),
      vectorLayout: z.literal("replicated"),
    }),
    discretization: spatialDiscretizationSchema,
    solver: z.object({
      adapter: z.literal("eqiora.reference"),
      algorithm: z.literal("conjugate-gradient"),
      preconditioner: z.literal("identity"),
      reduction: z.literal("reproducible"),
      relativeTolerance: z.number().finite().nonnegative(),
      absoluteTolerance: z.number().finite().nonnegative(),
      maximumIterations: z.number().int().positive(),
    }),
    placement: z.object({
      kind: z.literal("host"),
      adapter: z.enum(["eqiora.host.serial", "eqiora.rayon"]),
      workers: z.number().int().positive().max(64),
      maximumWorkers: z.number().int().positive().max(64),
      budgetSource: z.literal("studio-session-budget"),
    }),
    limits: z.object({
      maximumEntityCount: z.literal(MAX_SPATIAL_ENTITY_COUNT),
    }),
    acceptance: z.object({
      algebraic: z.literal("independent-true-residual"),
      continuous: z.literal("boundary-source-balance"),
      independentTrueResidual: z.literal(true),
    }),
  })
  .superRefine((plan, context) => {
    if (plan.placement.workers > plan.placement.maximumWorkers) {
      context.addIssue({
        code: "custom",
        message: "Spatial placement exceeds its admitted worker budget.",
        path: ["placement", "workers"],
      });
    }
    const expectedAdapter = plan.placement.workers === 1 ? "eqiora.host.serial" : "eqiora.rayon";
    if (plan.placement.adapter !== expectedAdapter) {
      context.addIssue({
        code: "custom",
        message: "Spatial placement adapter contradicts its worker count.",
        path: ["placement", "adapter"],
      });
    }
  });

export type SpatialRunPlan = z.infer<typeof spatialRunPlanSchema>;

const executionTopologySchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("host"), workers: z.number().int().positive() }),
  z.object({ kind: z.literal("distributed"), ranks: z.number().int().positive() }),
  z.object({ kind: z.literal("cuda"), device: z.number().int().nonnegative().max(65_535) }),
]);

const executionEvidenceSchema = z.object({
  adapter: z.string().min(1).max(128),
  topology: executionTopologySchema,
});

export const spatialRunResultSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    runId: z.string().uuid(),
    digest: z.string().min(16).max(128),
    plan: spatialRunPlanSchema,
    elapsedSeconds: z.number().finite().nonnegative(),
    field: z.object({
      location: z.enum(["vertex", "cell-center"]),
      valueCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
      minimum: z.number().finite(),
      maximum: z.number().finite(),
    }),
    balance: z.object({
      boundaryTotal: z.number().finite(),
      integratedSource: z.number().finite(),
      relativeImbalance: z.number().finite().nonnegative(),
    }),
    assembly: z.object({
      execution: executionEvidenceSchema,
      packetCount: z.number().int().nonnegative(),
      targetCount: z.number().int().positive(),
    }),
    solve: z.object({
      backend: z.string().min(1).max(128),
      execution: executionEvidenceSchema,
      verification: executionEvidenceSchema,
      algorithm: z.enum(["conjugate-gradient", "bicgstab"]),
      preconditioner: z.enum(["identity", "jacobi"]),
      reduction: z.enum(["reproducible", "fast"]),
      reason: z.enum(["initial-residual-satisfied", "residual-tolerance-satisfied"]),
      completedIterations: z.number().int().nonnegative(),
      initialResidualNorm: z.number().finite().nonnegative(),
      reportedResidualNorm: z.number().finite().nonnegative(),
      trueResidualNorm: z.number().finite().nonnegative(),
      residualTarget: z.number().finite().nonnegative(),
    }),
  })
  .superRefine((result, context) => {
    if (result.digest !== result.plan.modelDigest) {
      context.addIssue({
        code: "custom",
        message: "Spatial result and Realization plan identify different models.",
        path: ["plan", "modelDigest"],
      });
    }
    if (result.field.valueCount !== result.plan.discretization.fieldValueCount) {
      context.addIssue({
        code: "custom",
        message: "Spatial result shape differs from the allocation-free preview.",
        path: ["field", "valueCount"],
      });
    }
    const expectedLocation =
      result.plan.discretization.method === "finite-element" ? "vertex" : "cell-center";
    if (result.field.location !== expectedLocation) {
      context.addIssue({
        code: "custom",
        message: "Spatial field location contradicts its numerical method.",
        path: ["field", "location"],
      });
    }
    if (result.field.maximum < result.field.minimum) {
      context.addIssue({
        code: "custom",
        message: "Spatial field range is inverted.",
        path: ["field", "maximum"],
      });
    }
    if (result.solve.trueResidualNorm > result.solve.residualTarget) {
      context.addIssue({
        code: "custom",
        message: "Spatial solve evidence did not satisfy its true-residual target.",
        path: ["solve", "trueResidualNorm"],
      });
    }
  });

export type SpatialRunResult = z.infer<typeof spatialRunResultSchema>;

export const runPlanSchema = z.object({
  protocol: z.literal(BRIDGE_PROTOCOL),
  key: z.string().min(1).max(256),
  adapter: z.object({
    id: z.string().min(1).max(128),
    version: z.string().min(1).max(64),
  }),
  placement: z.object({
    kind: z.literal("host"),
    workers: z.number().int().positive().max(65_536),
  }),
  integration: z.object({
    method: z.literal("backward-euler"),
    endTime: z.number().finite().positive(),
    maxStep: z.number().finite().positive(),
  }),
  nonlinear: z.object({
    method: z.literal("dense-finite-difference-newton"),
    absoluteTolerance: z.number().finite().positive(),
    relativeTolerance: z.number().finite().nonnegative(),
    maximumIterations: z.number().int().positive(),
  }),
  events: z.object({
    timeTolerance: z.number().finite().positive(),
    guardTolerance: z.number().finite().positive(),
    maximumLocalizationIterations: z.number().int().positive(),
    maximumZeroTimeEvents: z.number().int().positive(),
  }),
  limits: z.object({
    maximumSteps: z.number().int().positive(),
  }),
  acceptance: z.object({
    kind: z.literal("semantic-oracle"),
    independentVerifier: z.literal(false),
  }),
});

export type RunPlan = z.infer<typeof runPlanSchema>;

export const runEvidenceSchema = z.object({
  plan: runPlanSchema,
  elapsedSeconds: z.number().finite().nonnegative(),
  fieldCount: z.number().int().nonnegative(),
  sampleCount: z.number().int().nonnegative(),
});

export type RunEvidence = z.infer<typeof runEvidenceSchema>;

export const resultSeriesSchema = z.object({
  fieldId: z.string().min(1).max(128),
  name: z.string().min(1).max(512),
  dimension: z.string().max(128),
  time: z.array(z.number().finite()).max(10_000_000),
  values: z.array(z.number().finite()).max(10_000_000),
});

export const runResultSchema = z.object({
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  evidence: runEvidenceSchema,
  series: z.array(resultSeriesSchema).max(100_000),
});

export type RunResult = z.infer<typeof runResultSchema>;

export const runProgressSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    runId: z.string().uuid(),
    modelTime: z.number().finite().nonnegative(),
    endTime: z.number().finite().positive(),
    acceptedSteps: z.number().int().nonnegative(),
    maximumSteps: z.number().int().positive(),
    elapsedSeconds: z.number().finite().nonnegative(),
  })
  .refine((progress) => progress.modelTime <= progress.endTime, {
    message: "Accepted model time cannot exceed the requested end time.",
    path: ["modelTime"],
  })
  .refine((progress) => progress.acceptedSteps <= progress.maximumSteps, {
    message: "Accepted steps cannot exceed the run safety limit.",
    path: ["acceptedSteps"],
  });

export type RunProgress = z.infer<typeof runProgressSchema>;

export const runCancellationSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    runId: z.string().uuid(),
    plan: runPlanSchema,
    elapsedSeconds: z.number().finite().nonnegative(),
    progress: runProgressSchema,
  })
  .refine((cancellation) => cancellation.progress.runId === cancellation.runId, {
    message: "Cancellation and progress must identify the same run.",
    path: ["progress", "runId"],
  })
  .refine(
    (cancellation) =>
      cancellation.progress.endTime === cancellation.plan.integration.endTime &&
      cancellation.elapsedSeconds >= cancellation.progress.elapsedSeconds,
    {
      message: "Cancellation evidence does not match its accepted plan or progress boundary.",
      path: ["progress"],
    },
  );

export type RunCancellation = z.infer<typeof runCancellationSchema>;

export const runOutcomeSchema = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("completed"), result: runResultSchema }),
  z.object({ kind: z.literal("cancelled"), cancellation: runCancellationSchema }),
]);

export type RunOutcome = z.infer<typeof runOutcomeSchema>;

export const cancelRunResultSchema = z.object({
  protocol: z.literal(BRIDGE_PROTOCOL),
  runId: z.string().uuid(),
  status: z.enum(["requested", "already-terminal", "not-cancellable"]),
});

export type CancelRunResult = z.infer<typeof cancelRunResultSchema>;

export function bridgeEnvelopeSchema<T extends z.ZodType>(result: T) {
  return z.object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    result: result.nullable(),
    diagnostics: z.array(diagnosticSchema).max(10_000),
  });
}

export type BridgeEnvelope<T> = {
  protocol: typeof BRIDGE_PROTOCOL;
  result: T | null;
  diagnostics: StudioDiagnostic[];
};
