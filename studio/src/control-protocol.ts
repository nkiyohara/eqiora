import { z } from "zod";
import compileV1SchemaDocument from "../../schemas/control/compile-v1.schema.json";

export const CONTROL_PROTOCOL_V1 = "eqiora.control/v1" as const;
export const COMPILE_COMMAND_V1 = "model.compile-check/v1" as const;
export const CURRENT_AUTHORING_MODEL_WIRE = "v8" as const;
export const STUDIO_REQUIRED_COMPILE_FEATURES = [
  COMPILE_COMMAND_V1,
  `model-wire/${CURRENT_AUTHORING_MODEL_WIRE}`,
] as const;

const MAX_FILENAME_UTF8_BYTES =
  compileV1SchemaDocument.$defs.request.properties.filename["x-eqiora-maxUtf8Bytes"];
const MAX_SOURCE_UTF8_BYTES =
  compileV1SchemaDocument.$defs.request.properties.source["x-eqiora-maxUtf8Bytes"];

const requestIdSchema = z
  .string()
  .min(1)
  .max(128)
  .regex(/^[A-Za-z0-9._:-]+$/);
const modelWireSchema = z.enum(["v1", "v2", "v3", "v4", "v5", "v6", "v7", "v8"]);
const featureSchema = z.enum([
  COMPILE_COMMAND_V1,
  "model-wire/v1",
  "model-wire/v2",
  "model-wire/v3",
  "model-wire/v4",
  "model-wire/v5",
  "model-wire/v6",
  "model-wire/v7",
  "model-wire/v8",
]);
const requiredFeaturesSchema = z.array(featureSchema).max(16);
const utf8 = new TextEncoder();

function utf8Bytes(value: string): number {
  return utf8.encode(value).length;
}

const filenameSchema = z
  .string()
  .refine(
    (filename) => utf8Bytes(filename) >= 1 && utf8Bytes(filename) <= MAX_FILENAME_UTF8_BYTES,
    {
      message: "Filename must contain 1 to 4096 UTF-8 bytes.",
    },
  )
  .refine((filename) => !/\p{Cc}/u.test(filename), {
    message: "Filename cannot contain control characters.",
  });

const sourceSchema = z.string().refine((source) => utf8Bytes(source) <= MAX_SOURCE_UTF8_BYTES, {
  message: "Source exceeds the 8 MiB UTF-8 compile/check limit.",
});

function requestFeaturesMatchWire(
  features: readonly string[],
  wire: z.infer<typeof modelWireSchema>,
) {
  const normalized = new Set(features);
  return (
    normalized.size === 2 &&
    normalized.has(COMPILE_COMMAND_V1) &&
    normalized.has(`model-wire/${wire}`)
  );
}

function responseFeaturesMatchWire(
  features: readonly string[],
  wire: z.infer<typeof modelWireSchema>,
) {
  return (
    features.length === 2 &&
    features[0] === COMPILE_COMMAND_V1 &&
    features[1] === `model-wire/${wire}`
  );
}

export const compileRequestV1Schema = z
  .object({
    protocol: z.literal(CONTROL_PROTOCOL_V1),
    command: z.literal(COMPILE_COMMAND_V1),
    requestId: requestIdSchema,
    requiredFeatures: requiredFeaturesSchema,
    modelWire: modelWireSchema,
    filename: filenameSchema,
    source: sourceSchema,
  })
  .strict()
  .refine((request) => requestFeaturesMatchWire(request.requiredFeatures, request.modelWire), {
    message: "Required compile/check features must exactly match the selected Model wire.",
    path: ["requiredFeatures"],
  });

export type CompileRequestV1 = z.infer<typeof compileRequestV1Schema>;

const controlSourceSpanV1Schema = z
  .object({
    file: filenameSchema,
    start: z.number().int().min(0).max(4_294_967_295),
    end: z.number().int().min(0).max(4_294_967_295),
  })
  .strict()
  .refine((span) => span.end >= span.start, {
    message: "Source span end must not precede its start.",
  });

export const controlDiagnosticV1Schema = z
  .object({
    source: z.enum(["control", "kernel"]),
    severity: z.enum(["error", "warning", "note"]),
    code: z.string().regex(/^[A-Z]{2}[0-9]{4}$/),
    message: z.string().min(1),
    graphPath: z.array(z.string()).nullable(),
    span: controlSourceSpanV1Schema.nullable(),
    patch: z
      .object({ summary: z.string().min(1) })
      .strict()
      .nullable(),
  })
  .strict();

export type ControlDiagnosticV1 = z.infer<typeof controlDiagnosticV1Schema>;

const compileModelDescriptorV1Schema = z
  .object({
    wire: modelWireSchema,
    schema: z.enum([
      "eqiora.model-envelope/v1",
      "eqiora.model-envelope/v2",
      "eqiora.model-envelope/v3",
      "eqiora.model-envelope/v4",
      "eqiora.model-envelope/v5",
      "eqiora.model-envelope/v6",
      "eqiora.model-envelope/v7",
      "eqiora.model-envelope/v8",
    ]),
    digest: z.string().regex(/^[0-9a-f]{64}$/),
    modelId: z.string().min(1).max(128),
    semanticRevision: z.number().int().nonnegative(),
  })
  .strict()
  .refine((model) => model.schema === `eqiora.model-envelope/${model.wire}`, {
    message: "Accepted Model schema and wire differ.",
    path: ["schema"],
  });

const compileOutcomeV1Schema = z.discriminatedUnion("status", [
  z.object({ status: z.literal("accepted"), model: compileModelDescriptorV1Schema }).strict(),
  z
    .object({
      status: z.literal("rejected"),
      diagnostics: z.array(controlDiagnosticV1Schema).min(1),
    })
    .strict(),
]);

export const compileResponseV1Schema = z
  .object({
    protocol: z.literal(CONTROL_PROTOCOL_V1),
    command: z.literal(COMPILE_COMMAND_V1),
    requestId: requestIdSchema,
    requiredFeatures: requiredFeaturesSchema,
    modelWire: modelWireSchema,
    outcome: compileOutcomeV1Schema,
  })
  .strict()
  .refine((response) => responseFeaturesMatchWire(response.requiredFeatures, response.modelWire), {
    message: "Response features must exactly match the selected Model wire.",
    path: ["requiredFeatures"],
  });

export type CompileResponseV1 = z.infer<typeof compileResponseV1Schema>;

export function studioCompileRequest(requestId: string, filename: string, source: string) {
  return compileRequestV1Schema.parse({
    protocol: CONTROL_PROTOCOL_V1,
    command: COMPILE_COMMAND_V1,
    requestId,
    requiredFeatures: STUDIO_REQUIRED_COMPILE_FEATURES,
    modelWire: CURRENT_AUTHORING_MODEL_WIRE,
    filename,
    source,
  });
}

export function compileResponseMatchesRequest(
  request: CompileRequestV1,
  response: CompileResponseV1,
): boolean {
  return (
    response.protocol === request.protocol &&
    response.command === request.command &&
    response.requestId === request.requestId &&
    response.modelWire === request.modelWire &&
    responseFeaturesMatchWire(response.requiredFeatures, request.modelWire)
  );
}

// The committed JSON Schema is the language-neutral source of this wire. This
// narrow assertion makes accidental replacement or opening of the schema fail
// at Studio startup; unit tests bind its complete request/response field sets.
const schemaIdentity = z
  .object({
    $id: z.literal("urn:eqiora:schema:control:compile-v1"),
    $defs: z.object({
      modelWire: z.object({ enum: z.array(z.string()) }).passthrough(),
      request: z.object({ additionalProperties: z.literal(false) }).passthrough(),
      requiredFeatures: z
        .object({ items: z.object({ enum: z.array(z.string()) }).passthrough() })
        .passthrough(),
      response: z.object({ additionalProperties: z.literal(false) }).passthrough(),
    }),
  })
  .passthrough();

export const COMPILE_V1_SCHEMA_DOCUMENT = schemaIdentity.parse(compileV1SchemaDocument);
