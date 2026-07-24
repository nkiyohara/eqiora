import { z } from "zod";
import { diagnosticSchema } from "./protocol";
import { MAX_SPATIAL_ENTITY_COUNT, type SpatialRunResult } from "./spatial-protocol";

/** Closed data-plane protocol for the first bounded two-dimensional scalar-field view. */
export const SCALAR_FIELD_VIEW_PROTOCOL = "eqiora.studio.field-view/v1" as const;
export const SCALAR_FIELD_VALUES_PER_CHUNK = 4_096;
export const SCALAR_FIELD_MAX_CHUNK_COUNT = Math.ceil(
  MAX_SPATIAL_ENTITY_COUNT / SCALAR_FIELD_VALUES_PER_CHUNK,
);

const modelDigestSchema = z.string().min(16).max(128);
const planKeySchema = z.string().regex(/^[0-9a-f]{64}$/);
const runIdSchema = z.string().uuid();
const semanticIdSchema = z.string().min(1).max(128);
const semanticTextSchema = z.string().min(1).max(128);
const presentationNameSchema = z.string().min(1).max(512);
const finiteIntervalSchema = z
  .tuple([z.number().finite(), z.number().finite()])
  .refine(([lower, upper]) => upper > lower, "Field-view interval must have positive extent.");

export const scalarFieldOpenRequestSchema = z
  .object({
    protocol: z.literal(SCALAR_FIELD_VIEW_PROTOCOL),
    modelDigest: modelDigestSchema,
    runId: runIdSchema,
    planKey: planKeySchema,
  })
  .strict();

export type ScalarFieldOpenRequest = z.infer<typeof scalarFieldOpenRequestSchema>;

export const scalarFieldDescriptorSchema = z
  .object({
    protocol: z.literal(SCALAR_FIELD_VIEW_PROTOCOL),
    modelDigest: modelDigestSchema,
    runId: runIdSchema,
    planKey: planKeySchema,
    field: z
      .object({
        id: semanticIdSchema,
        name: presentationNameSchema,
        dimension: semanticTextSchema,
        coherentSiUnit: semanticTextSchema,
        scalarType: z.literal("f64"),
        location: z.enum(["vertex", "cell-center"]),
        valueCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
        minimum: z.number().finite(),
        maximum: z.number().finite(),
      })
      .strict(),
    domain: z
      .object({
        id: semanticIdSchema,
        boundsM: z.tuple([finiteIntervalSchema, finiteIntervalSchema]),
      })
      .strict(),
    grid: z
      .object({
        kind: z.literal("uniform-cartesian-2d"),
        logicalShape: z.tuple([
          z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
          z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
        ]),
        order: z.literal("row-major-last-axis-fastest"),
      })
      .strict(),
    transport: z
      .object({
        kind: z.literal("explicit-owned-host-copy"),
        encoding: z.literal("f64-le"),
        valuesPerChunk: z.literal(SCALAR_FIELD_VALUES_PER_CHUNK),
        chunkCount: z.number().int().positive().max(SCALAR_FIELD_MAX_CHUNK_COUNT),
      })
      .strict(),
  })
  .strict()
  .superRefine((descriptor, context) => {
    if (descriptor.field.maximum < descriptor.field.minimum) {
      context.addIssue({
        code: "custom",
        message: "Scalar-field range is inverted.",
        path: ["field", "maximum"],
      });
    }
    if (
      descriptor.field.valueCount === 1 &&
      !Object.is(descriptor.field.minimum, descriptor.field.maximum)
    ) {
      context.addIssue({
        code: "custom",
        message: "A one-value scalar field cannot claim two distinct extrema.",
        path: ["field"],
      });
    }

    const [rows, columns] = descriptor.grid.logicalShape;
    const valueCount = rows * columns;
    if (!Number.isSafeInteger(valueCount) || valueCount !== descriptor.field.valueCount) {
      context.addIssue({
        code: "custom",
        message: "Scalar-field logical shape and value count differ.",
        path: ["grid", "logicalShape"],
      });
    }

    const chunkCount = Math.ceil(descriptor.field.valueCount / descriptor.transport.valuesPerChunk);
    if (chunkCount !== descriptor.transport.chunkCount) {
      context.addIssue({
        code: "custom",
        message: "Scalar-field transport chunk count is not canonical.",
        path: ["transport", "chunkCount"],
      });
    }
  });

export type ScalarFieldDescriptor = z.infer<typeof scalarFieldDescriptorSchema>;

export const scalarFieldOpenEnvelopeSchema = z
  .object({
    protocol: z.literal(SCALAR_FIELD_VIEW_PROTOCOL),
    result: scalarFieldDescriptorSchema.nullable(),
    diagnostics: z.array(diagnosticSchema).max(10_000),
  })
  .strict()
  .superRefine((envelope, context) => {
    const hasError = envelope.diagnostics.some((diagnostic) => diagnostic.severity === "error");
    if ((envelope.result === null) !== hasError) {
      context.addIssue({
        code: "custom",
        message:
          "Field-view result presence and error diagnostics must describe one terminal outcome.",
        path: envelope.result === null ? ["diagnostics"] : ["result"],
      });
    }
  });

export type ScalarFieldOpenEnvelope = z.infer<typeof scalarFieldOpenEnvelopeSchema>;

export const scalarFieldFailureEnvelopeSchema = z
  .object({
    protocol: z.literal(SCALAR_FIELD_VIEW_PROTOCOL),
    result: z.null(),
    diagnostics: z
      .array(diagnosticSchema)
      .min(1)
      .max(10_000)
      .refine(
        (diagnostics) => diagnostics.some((diagnostic) => diagnostic.severity === "error"),
        "Field-view failure must contain an error diagnostic.",
      ),
  })
  .strict();

export const scalarFieldChunkRequestSchema = z
  .object({
    protocol: z.literal(SCALAR_FIELD_VIEW_PROTOCOL),
    modelDigest: modelDigestSchema,
    runId: runIdSchema,
    planKey: planKeySchema,
    chunkIndex: z
      .number()
      .int()
      .nonnegative()
      .max(SCALAR_FIELD_MAX_CHUNK_COUNT - 1),
  })
  .strict();

export type ScalarFieldChunkRequest = z.infer<typeof scalarFieldChunkRequestSchema>;

export function scalarFieldOpenRequest(result: SpatialRunResult): ScalarFieldOpenRequest {
  return {
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: result.digest,
    runId: result.runId,
    planKey: result.plan.key,
  };
}

export function scalarFieldChunkRequest(
  descriptor: ScalarFieldDescriptor,
  chunkIndex: number,
): ScalarFieldChunkRequest {
  return {
    protocol: SCALAR_FIELD_VIEW_PROTOCOL,
    modelDigest: descriptor.modelDigest,
    runId: descriptor.runId,
    planKey: descriptor.planKey,
    chunkIndex,
  };
}

export function scalarFieldChunkValueCount(
  descriptor: ScalarFieldDescriptor,
  chunkIndex: number,
): number | null {
  if (
    !Number.isSafeInteger(chunkIndex) ||
    chunkIndex < 0 ||
    chunkIndex >= descriptor.transport.chunkCount
  ) {
    return null;
  }
  const offset = chunkIndex * descriptor.transport.valuesPerChunk;
  return Math.min(descriptor.transport.valuesPerChunk, descriptor.field.valueCount - offset);
}

/**
 * Bind a data-plane descriptor to the exact accepted result which authorized it.
 * Geometry and field-family identity remain deliberately bounded by this first workflow.
 */
export function descriptorMatchesAcceptedResult(
  result: SpatialRunResult,
  descriptor: ScalarFieldDescriptor,
): boolean {
  if (
    descriptor.modelDigest !== result.digest ||
    descriptor.runId !== result.runId ||
    descriptor.planKey !== result.plan.key ||
    result.plan.modelDigest !== result.digest ||
    result.plan.requirements.spatialDimension !== 2 ||
    descriptor.field.valueCount !== result.field.valueCount ||
    descriptor.field.location !== result.field.location ||
    !Object.is(descriptor.field.minimum, result.field.minimum) ||
    !Object.is(descriptor.field.maximum, result.field.maximum)
  ) {
    return false;
  }

  const cellsPerAxis = result.plan.discretization.cellsPerAxis;
  const finiteElement = result.plan.discretization.method === "finite-element";
  const expectedAxis = finiteElement ? cellsPerAxis + 1 : cellsPerAxis;
  const expectedLocation = finiteElement ? "vertex" : "cell-center";
  const [rows, columns] = descriptor.grid.logicalShape;

  return (
    descriptor.field.location === expectedLocation &&
    rows === expectedAxis &&
    columns === expectedAxis
  );
}
