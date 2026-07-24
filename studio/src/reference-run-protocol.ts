import { z } from "zod";
import { BRIDGE_PROTOCOL } from "./protocol";

export const MAX_REQUESTED_STEPS = 5_000_000;

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
