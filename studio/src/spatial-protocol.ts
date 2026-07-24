import { z } from "zod";
import { BRIDGE_PROTOCOL } from "./protocol";

export const MAX_SPATIAL_ENTITY_COUNT = 250_000;

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
