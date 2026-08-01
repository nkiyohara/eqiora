import { describe, expect, it } from "vitest";
import expectedContract from "../../verify/interfaces/control-plane-compile-check/expected/contract.json";
import acceptedFixture from "../../verify/interfaces/control-plane-compile-check/models/accepted-v2.json";
import forbiddenModelWireFixture from "../../verify/interfaces/control-plane-compile-check/models/forbidden-model-wire-v2.json";
import forbiddenRequiredFeaturesFixture from "../../verify/interfaces/control-plane-compile-check/models/forbidden-required-features-v2.json";
import rejectedSourceFixture from "../../verify/interfaces/control-plane-compile-check/models/rejected-source-v2.json";
import retiredProtocolFixture from "../../verify/interfaces/control-plane-compile-check/models/retired-v1.json";
import unknownCommandFixture from "../../verify/interfaces/control-plane-compile-check/models/unknown-command-v2.json";
import unknownProtocolFixture from "../../verify/interfaces/control-plane-compile-check/models/unknown-protocol-v2.json";
import {
  COMPILE_COMMAND_V1,
  COMPILE_V2_SCHEMA_DOCUMENT,
  CONTROL_PROTOCOL_V2,
  CURRENT_MODEL_SCHEMA,
  CURRENT_MODEL_TRANSACTION_SCHEMA,
  compileRequestV2Schema,
  compileResponseMatchesRequest,
  compileResponseV2Schema,
  studioCompileRequest,
} from "./control-protocol";

const REQUEST = studioCompileRequest("studio.compile:7", "model.eqi", "model empty {}\n");
const ACCEPTED = {
  protocol: CONTROL_PROTOCOL_V2,
  command: COMPILE_COMMAND_V1,
  requestId: REQUEST.requestId,
  outcome: {
    status: "accepted",
    model: {
      schema: CURRENT_MODEL_SCHEMA,
      transactionSchema: CURRENT_MODEL_TRANSACTION_SCHEMA,
      digest: "a".repeat(64),
      modelId: "Model:example",
      semanticRevision: 1,
    },
  },
} as const;

describe("compile/check control protocol v2", () => {
  it("constructs the closed current-only request without caller policy", () => {
    expect(Object.keys(REQUEST)).toEqual([
      "protocol",
      "command",
      "requestId",
      "filename",
      "source",
    ]);
    expect(REQUEST.protocol).toBe(expectedContract.protocol);
    expect(REQUEST.command).toBe(expectedContract.command);
  });

  it("shares the registered request fixtures with Rust and Python clients", () => {
    const accepted = compileRequestV2Schema.parse(acceptedFixture);
    const rejectedSource = compileRequestV2Schema.parse(rejectedSourceFixture);

    expect(accepted.requestId).toBe(expectedContract.accepted.requestId);
    expect(expectedContract.rejections.map(({ requestId }) => requestId)).toContain(
      rejectedSource.requestId,
    );
    expect(compileRequestV2Schema.safeParse(retiredProtocolFixture).success).toBe(false);
    expect(compileRequestV2Schema.safeParse(unknownProtocolFixture).success).toBe(false);
    expect(compileRequestV2Schema.safeParse(unknownCommandFixture).success).toBe(false);
    expect(compileRequestV2Schema.safeParse(forbiddenModelWireFixture).success).toBe(false);
    expect(compileRequestV2Schema.safeParse(forbiddenRequiredFeaturesFixture).success).toBe(false);
  });

  it("is bound to the committed closed JSON Schema", () => {
    const requestSchema = COMPILE_V2_SCHEMA_DOCUMENT.$defs.request as {
      readonly additionalProperties: boolean;
      readonly required: readonly string[];
      readonly "x-eqiora-maxEncodedUtf8Bytes": number;
      readonly properties: {
        readonly filename: { readonly "x-eqiora-maxUtf8Bytes": number };
        readonly source: { readonly "x-eqiora-maxUtf8Bytes": number };
      };
    };
    const responseSchema = COMPILE_V2_SCHEMA_DOCUMENT.$defs.response as {
      readonly additionalProperties: boolean;
      readonly required: readonly string[];
    };
    const modelSchema = COMPILE_V2_SCHEMA_DOCUMENT.$defs.model as {
      readonly additionalProperties: boolean;
      readonly required: readonly string[];
      readonly properties: {
        readonly schema: { readonly const: string };
        readonly transactionSchema: { readonly const: string };
      };
    };

    expect(requestSchema.additionalProperties).toBe(false);
    expect(requestSchema["x-eqiora-maxEncodedUtf8Bytes"]).toBe(8 * 1_024 * 1_024 + 16 * 1_024);
    expect(requestSchema.properties.filename["x-eqiora-maxUtf8Bytes"]).toBe(4_096);
    expect(requestSchema.properties.source["x-eqiora-maxUtf8Bytes"]).toBe(8 * 1_024 * 1_024);
    expect(requestSchema.required).toEqual([
      "protocol",
      "command",
      "requestId",
      "filename",
      "source",
    ]);
    expect(responseSchema.additionalProperties).toBe(false);
    expect(responseSchema.required).toEqual(["protocol", "command", "requestId", "outcome"]);
    expect(modelSchema.additionalProperties).toBe(false);
    expect(modelSchema.required).toEqual([
      "schema",
      "transactionSchema",
      "digest",
      "modelId",
      "semanticRevision",
    ]);
    expect(modelSchema.properties.schema.const).toBe(CURRENT_MODEL_SCHEMA);
    expect(modelSchema.properties.transactionSchema.const).toBe(CURRENT_MODEL_TRANSACTION_SCHEMA);
  });

  it("rejects unknown fields, retired negotiation members, and bounded-text drift", () => {
    expect(compileRequestV2Schema.safeParse({ ...REQUEST, extra: true }).success).toBe(false);
    expect(compileRequestV2Schema.safeParse({ ...REQUEST, modelWire: "v8" }).success).toBe(false);
    expect(
      compileRequestV2Schema.safeParse({
        ...REQUEST,
        requiredFeatures: [COMPILE_COMMAND_V1, "model-wire/v8"],
      }).success,
    ).toBe(false);
    expect(
      compileRequestV2Schema.safeParse({ ...REQUEST, filename: `${"界".repeat(1_366)}x` }).success,
    ).toBe(false);
    expect(
      compileRequestV2Schema.safeParse({ ...REQUEST, filename: "bad\nname.eqi" }).success,
    ).toBe(false);
    expect(
      compileRequestV2Schema.safeParse({ ...REQUEST, requestId: "x".repeat(129) }).success,
    ).toBe(false);
  });

  it("accepts only the fixed current descriptor for the exact request identity", () => {
    const response = compileResponseV2Schema.parse(ACCEPTED);
    expect(compileResponseMatchesRequest(REQUEST, response)).toBe(true);
    expect(
      compileResponseMatchesRequest(REQUEST, { ...response, requestId: "studio.compile:8" }),
    ).toBe(false);
    expect(compileResponseV2Schema.safeParse({ ...ACCEPTED, source: "leak" }).success).toBe(false);
    expect(
      compileResponseV2Schema.safeParse({
        ...ACCEPTED,
        outcome: {
          ...ACCEPTED.outcome,
          model: { ...ACCEPTED.outcome.model, transactionSchema: "unknown" },
        },
      }).success,
    ).toBe(false);
    expect(
      compileResponseV2Schema.safeParse({
        ...ACCEPTED,
        outcome: {
          ...ACCEPTED.outcome,
          model: { ...ACCEPTED.outcome.model, modelId: "界".repeat(128) },
        },
      }).success,
    ).toBe(true);
    expect(
      compileResponseV2Schema.safeParse({
        ...ACCEPTED,
        outcome: {
          ...ACCEPTED.outcome,
          model: { ...ACCEPTED.outcome.model, modelId: "界".repeat(129) },
        },
      }).success,
    ).toBe(false);
  });

  it("preserves bounded structured diagnostics without broadening the response", () => {
    const rejected = compileResponseV2Schema.parse({
      ...ACCEPTED,
      outcome: {
        status: "rejected",
        diagnostics: [
          {
            source: "kernel",
            severity: "error",
            code: "EQ0301",
            message: "invalid relation",
            graphPath: ["Model:example", "Relation:bad"],
            span: { file: "model.eqi", start: 6, end: 7 },
            patch: { summary: "Remove the invalid token." },
          },
        ],
      },
    });

    expect(rejected.outcome).toEqual({
      status: "rejected",
      diagnostics: [
        {
          source: "kernel",
          severity: "error",
          code: "EQ0301",
          message: "invalid relation",
          graphPath: ["Model:example", "Relation:bad"],
          span: { file: "model.eqi", start: 6, end: 7 },
          patch: { summary: "Remove the invalid token." },
        },
      ],
    });
    expect(
      compileResponseV2Schema.safeParse({
        ...ACCEPTED,
        outcome: {
          status: "rejected",
          diagnostics: [
            {
              source: "kernel",
              severity: "error",
              code: "EQ0301",
              message: "",
              graphPath: null,
              span: null,
              patch: null,
            },
          ],
        },
      }).success,
    ).toBe(false);
  });
});
