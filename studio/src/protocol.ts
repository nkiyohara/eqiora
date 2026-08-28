import { z } from "zod";

export const BRIDGE_PROTOCOL = "eqiora.studio.bridge/v5" as const;
export const MAX_PROJECTION_NODE_COUNT = 100_000;
export const MAX_PROJECTION_EDGE_COUNT = 400_000;
export const artifactDigestSchema = z.string().regex(/^[0-9a-f]{64}$/);

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
});

export type DocumentProjection = z.infer<typeof documentProjectionSchema>;

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
