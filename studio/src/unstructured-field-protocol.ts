import { z } from "zod";
import { diagnosticSchema } from "./protocol";
import { MAX_SPATIAL_ENTITY_COUNT } from "./spatial-protocol";

/** Closed protocol for one bounded affine-triangle P1 scalar view. */
export const UNSTRUCTURED_FIELD_VIEW_PROTOCOL = "eqiora.studio.unstructured-field-view/v1" as const;
export const UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK = 4_096;
export const UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT = MAX_SPATIAL_ENTITY_COUNT * 2;

export const artifactDigestSchema = z.string().regex(/^[0-9a-f]{64}$/);
const semanticRevisionSchema = z
  .string()
  .regex(/^(0|[1-9][0-9]{0,19})$/)
  .refine(
    (revision) => revision.length < 20 || revision <= "18446744073709551615",
    "Semantic revision exceeds u64.",
  );
const semanticIdSchema = z.string().min(1).max(128);
const semanticTextSchema = z.string().min(1).max(128);
const finiteIntervalSchema = z
  .tuple([z.number().finite(), z.number().finite()])
  .refine(([lower, upper]) => upper > lower, "Field-view interval must have positive extent.");
const artifactIdentitySchema = z
  .object({
    modelDigest: artifactDigestSchema,
    semanticRevision: semanticRevisionSchema,
    realizationDigest: artifactDigestSchema,
    runDigest: artifactDigestSchema,
    snapshotDigest: artifactDigestSchema,
    meshDigest: artifactDigestSchema,
  })
  .strict();
const projectionIdentitySchema = artifactIdentitySchema
  .extend({
    fieldId: semanticIdSchema,
    domainId: semanticIdSchema,
  })
  .strict();

const fieldSummarySchema = z
  .object({
    id: semanticIdSchema,
    dimension: semanticTextSchema,
    coherentSiUnit: semanticTextSchema,
    scalarType: z.literal("f64"),
    location: z.literal("vertex"),
    valueCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
    minimum: z.number().finite(),
    maximum: z.number().finite(),
  })
  .strict();
const domainSummarySchema = z
  .object({
    id: semanticIdSchema,
    boundsM: z.tuple([finiteIntervalSchema, finiteIntervalSchema]),
  })
  .strict();
const meshSummarySchema = z
  .object({
    kind: z.literal("affine-triangle-2d"),
    vertexCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
    triangleCount: z.number().int().positive().max(UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT),
  })
  .strict();

export const unstructuredFieldContextSchema = artifactIdentitySchema
  .extend({
    field: fieldSummarySchema.omit({ scalarType: true, location: true }),
    domain: domainSummarySchema,
    mesh: meshSummarySchema,
  })
  .strict()
  .superRefine((context, refinement) => {
    validateSummary(context, refinement);
  });

export type UnstructuredFieldContext = z.infer<typeof unstructuredFieldContextSchema>;

export const unstructuredFieldOpenRequestSchema = projectionIdentitySchema
  .extend({
    protocol: z.literal(UNSTRUCTURED_FIELD_VIEW_PROTOCOL),
  })
  .strict();

export type UnstructuredFieldOpenRequest = z.infer<typeof unstructuredFieldOpenRequestSchema>;

const coordinateStreamSchema = z
  .object({
    encoding: z.literal("f64-le"),
    components: z.literal(2),
    itemCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
    itemsPerChunk: z.literal(UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK),
    chunkCount: z.number().int().positive(),
  })
  .strict();
const triangleStreamSchema = z
  .object({
    encoding: z.literal("u32-le"),
    components: z.literal(3),
    itemCount: z.number().int().positive().max(UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT),
    itemsPerChunk: z.literal(UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK),
    chunkCount: z.number().int().positive(),
  })
  .strict();
const valueStreamSchema = z
  .object({
    encoding: z.literal("f64-le"),
    components: z.literal(1),
    itemCount: z.number().int().positive().max(MAX_SPATIAL_ENTITY_COUNT),
    itemsPerChunk: z.literal(UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK),
    chunkCount: z.number().int().positive(),
  })
  .strict();

export const unstructuredFieldDescriptorSchema = artifactIdentitySchema
  .extend({
    protocol: z.literal(UNSTRUCTURED_FIELD_VIEW_PROTOCOL),
    field: fieldSummarySchema,
    domain: domainSummarySchema,
    mesh: meshSummarySchema,
    transport: z
      .object({
        kind: z.literal("explicit-owned-host-copy"),
        coordinates: coordinateStreamSchema,
        triangles: triangleStreamSchema,
        values: valueStreamSchema,
      })
      .strict(),
  })
  .strict()
  .superRefine((descriptor, refinement) => {
    validateSummary(descriptor, refinement);
    const expectedStreams = [
      ["coordinates", descriptor.transport.coordinates, descriptor.mesh.vertexCount],
      ["triangles", descriptor.transport.triangles, descriptor.mesh.triangleCount],
      ["values", descriptor.transport.values, descriptor.field.valueCount],
    ] as const;
    for (const [name, stream, expectedItems] of expectedStreams) {
      if (stream.itemCount !== expectedItems) {
        refinement.addIssue({
          code: "custom",
          message: `Unstructured ${name} stream count differs from its projection.`,
          path: ["transport", name, "itemCount"],
        });
      }
      if (stream.chunkCount !== Math.ceil(stream.itemCount / stream.itemsPerChunk)) {
        refinement.addIssue({
          code: "custom",
          message: `Unstructured ${name} stream chunk count is not canonical.`,
          path: ["transport", name, "chunkCount"],
        });
      }
    }
  });

export type UnstructuredFieldDescriptor = z.infer<typeof unstructuredFieldDescriptorSchema>;
export type UnstructuredFieldStream = "coordinates" | "triangles" | "values";

export const unstructuredFieldOpenEnvelopeSchema = z
  .object({
    protocol: z.literal(UNSTRUCTURED_FIELD_VIEW_PROTOCOL),
    result: unstructuredFieldDescriptorSchema.nullable(),
    diagnostics: z.array(diagnosticSchema).max(10_000),
  })
  .strict()
  .superRefine((envelope, refinement) => {
    const hasError = envelope.diagnostics.some((diagnostic) => diagnostic.severity === "error");
    if ((envelope.result === null) !== hasError) {
      refinement.addIssue({
        code: "custom",
        message:
          "Unstructured field result presence and error diagnostics must describe one outcome.",
        path: envelope.result === null ? ["diagnostics"] : ["result"],
      });
    }
  });

export const unstructuredFieldFailureEnvelopeSchema = z
  .object({
    protocol: z.literal(UNSTRUCTURED_FIELD_VIEW_PROTOCOL),
    result: z.null(),
    diagnostics: z
      .array(diagnosticSchema)
      .min(1)
      .max(10_000)
      .refine(
        (diagnostics) => diagnostics.some((diagnostic) => diagnostic.severity === "error"),
        "Unstructured field failure must contain an error diagnostic.",
      ),
  })
  .strict();

export const unstructuredFieldChunkRequestSchema = projectionIdentitySchema
  .extend({
    protocol: z.literal(UNSTRUCTURED_FIELD_VIEW_PROTOCOL),
    stream: z.enum(["coordinates", "triangles", "values"]),
    chunkIndex: z.number().int().nonnegative(),
  })
  .strict();

export type UnstructuredFieldChunkRequest = z.infer<typeof unstructuredFieldChunkRequestSchema>;

export function unstructuredFieldOpenRequest(
  context: UnstructuredFieldContext,
): UnstructuredFieldOpenRequest {
  return {
    protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
    ...identityFromContext(context),
  };
}

export function unstructuredFieldChunkRequest(
  descriptor: UnstructuredFieldDescriptor,
  stream: UnstructuredFieldStream,
  chunkIndex: number,
): UnstructuredFieldChunkRequest {
  return {
    protocol: UNSTRUCTURED_FIELD_VIEW_PROTOCOL,
    ...identityFromDescriptor(descriptor),
    stream,
    chunkIndex,
  };
}

export function unstructuredFieldChunkItemCount(
  descriptor: UnstructuredFieldDescriptor,
  stream: UnstructuredFieldStream,
  chunkIndex: number,
): number | null {
  const contract = descriptor.transport[stream];
  if (!Number.isSafeInteger(chunkIndex) || chunkIndex < 0 || chunkIndex >= contract.chunkCount) {
    return null;
  }
  const offset = chunkIndex * contract.itemsPerChunk;
  return Math.min(contract.itemsPerChunk, contract.itemCount - offset);
}

export function unstructuredDescriptorMatchesContext(
  context: UnstructuredFieldContext,
  descriptor: UnstructuredFieldDescriptor,
): boolean {
  return (
    sameProjectionIdentity(identityFromContext(context), identityFromDescriptor(descriptor)) &&
    sameFieldSummary(context.field, descriptor.field) &&
    sameDomainSummary(context.domain, descriptor.domain) &&
    sameMeshSummary(context.mesh, descriptor.mesh)
  );
}

export function unstructuredFieldContextsEqual(
  left: UnstructuredFieldContext,
  right: UnstructuredFieldContext,
): boolean {
  return (
    sameProjectionIdentity(identityFromContext(left), identityFromContext(right)) &&
    sameFieldSummary(left.field, right.field) &&
    sameDomainSummary(left.domain, right.domain) &&
    sameMeshSummary(left.mesh, right.mesh)
  );
}

export function unstructuredFieldDescriptorsEqual(
  left: UnstructuredFieldDescriptor,
  right: UnstructuredFieldDescriptor,
): boolean {
  return (
    sameProjectionIdentity(identityFromDescriptor(left), identityFromDescriptor(right)) &&
    sameFieldSummary(left.field, right.field) &&
    sameDomainSummary(left.domain, right.domain) &&
    sameMeshSummary(left.mesh, right.mesh) &&
    left.protocol === right.protocol &&
    left.transport.kind === right.transport.kind &&
    sameStream(left.transport.coordinates, right.transport.coordinates) &&
    sameStream(left.transport.triangles, right.transport.triangles) &&
    sameStream(left.transport.values, right.transport.values)
  );
}

function validateSummary(
  summary: {
    readonly field: {
      readonly valueCount: number;
      readonly minimum: number;
      readonly maximum: number;
    };
    readonly mesh: { readonly vertexCount: number };
  },
  refinement: z.core.$RefinementCtx,
): void {
  if (summary.field.maximum < summary.field.minimum) {
    refinement.addIssue({
      code: "custom",
      message: "Unstructured scalar-field range is inverted.",
      path: ["field", "maximum"],
      input: summary,
    });
  }
  if (summary.field.valueCount !== summary.mesh.vertexCount) {
    refinement.addIssue({
      code: "custom",
      message: "Unstructured P1 Field requires exactly one value per mesh vertex.",
      path: ["field", "valueCount"],
      input: summary,
    });
  }
  if (summary.field.valueCount === 1 && !Object.is(summary.field.minimum, summary.field.maximum)) {
    refinement.addIssue({
      code: "custom",
      message: "A one-value unstructured Field cannot claim distinct extrema.",
      path: ["field"],
      input: summary,
    });
  }
}

function identityFromContext(context: UnstructuredFieldContext) {
  return {
    modelDigest: context.modelDigest,
    semanticRevision: context.semanticRevision,
    realizationDigest: context.realizationDigest,
    runDigest: context.runDigest,
    snapshotDigest: context.snapshotDigest,
    meshDigest: context.meshDigest,
    fieldId: context.field.id,
    domainId: context.domain.id,
  };
}

function identityFromDescriptor(descriptor: UnstructuredFieldDescriptor) {
  return {
    modelDigest: descriptor.modelDigest,
    semanticRevision: descriptor.semanticRevision,
    realizationDigest: descriptor.realizationDigest,
    runDigest: descriptor.runDigest,
    snapshotDigest: descriptor.snapshotDigest,
    meshDigest: descriptor.meshDigest,
    fieldId: descriptor.field.id,
    domainId: descriptor.domain.id,
  };
}

function sameProjectionIdentity(
  left: ReturnType<typeof identityFromContext>,
  right: ReturnType<typeof identityFromContext>,
): boolean {
  return (
    left.modelDigest === right.modelDigest &&
    left.semanticRevision === right.semanticRevision &&
    left.realizationDigest === right.realizationDigest &&
    left.runDigest === right.runDigest &&
    left.snapshotDigest === right.snapshotDigest &&
    left.meshDigest === right.meshDigest &&
    left.fieldId === right.fieldId &&
    left.domainId === right.domainId
  );
}

function sameFieldSummary(
  left: UnstructuredFieldContext["field"],
  right: UnstructuredFieldContext["field"],
): boolean {
  return (
    left.id === right.id &&
    left.dimension === right.dimension &&
    left.coherentSiUnit === right.coherentSiUnit &&
    left.valueCount === right.valueCount &&
    Object.is(left.minimum, right.minimum) &&
    Object.is(left.maximum, right.maximum)
  );
}

function sameDomainSummary(
  left: UnstructuredFieldContext["domain"],
  right: UnstructuredFieldContext["domain"],
): boolean {
  return (
    left.id === right.id &&
    Object.is(left.boundsM[0][0], right.boundsM[0][0]) &&
    Object.is(left.boundsM[0][1], right.boundsM[0][1]) &&
    Object.is(left.boundsM[1][0], right.boundsM[1][0]) &&
    Object.is(left.boundsM[1][1], right.boundsM[1][1])
  );
}

function sameMeshSummary(
  left: UnstructuredFieldContext["mesh"],
  right: UnstructuredFieldContext["mesh"],
): boolean {
  return (
    left.kind === right.kind &&
    left.vertexCount === right.vertexCount &&
    left.triangleCount === right.triangleCount
  );
}

function sameStream(
  left: {
    readonly encoding: string;
    readonly components: number;
    readonly itemCount: number;
    readonly itemsPerChunk: number;
    readonly chunkCount: number;
  },
  right: {
    readonly encoding: string;
    readonly components: number;
    readonly itemCount: number;
    readonly itemsPerChunk: number;
    readonly chunkCount: number;
  },
): boolean {
  return (
    left.encoding === right.encoding &&
    left.components === right.components &&
    left.itemCount === right.itemCount &&
    left.itemsPerChunk === right.itemsPerChunk &&
    left.chunkCount === right.chunkCount
  );
}
