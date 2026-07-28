import { describe, expect, it } from "vitest";
import expectedContract from "../../verify/interfaces/control-plane-compile-check/expected/contract.json";
import acceptedFixture from "../../verify/interfaces/control-plane-compile-check/models/accepted-v1.json";
import rejectedSourceFixture from "../../verify/interfaces/control-plane-compile-check/models/rejected-source-v1.json";
import unsupportedProtocolFixture from "../../verify/interfaces/control-plane-compile-check/models/unsupported-protocol-v1.json";
import currentProfile from "../../verify/interfaces/current-authoring-profile/expected/profile.json";
import {
  COMPILE_COMMAND_V1,
  COMPILE_V1_SCHEMA_DOCUMENT,
  CONTROL_PROTOCOL_V1,
  CURRENT_AUTHORING_MODEL_WIRE,
  compileRequestV1Schema,
  compileResponseMatchesRequest,
  compileResponseV1Schema,
  studioCompileRequest,
} from "./control-protocol";

const REQUEST = studioCompileRequest("studio.compile:7", "model.eqi", "model empty {}\n");
const ACCEPTED = {
  protocol: CONTROL_PROTOCOL_V1,
  command: COMPILE_COMMAND_V1,
  requestId: REQUEST.requestId,
  requiredFeatures: [...REQUEST.requiredFeatures],
  modelWire: REQUEST.modelWire,
  outcome: {
    status: "accepted",
    model: {
      wire: CURRENT_AUTHORING_MODEL_WIRE,
      schema: `eqiora.model-envelope/${CURRENT_AUTHORING_MODEL_WIRE}`,
      digest: "a".repeat(64),
      modelId: "Model:example",
      semanticRevision: 1,
    },
  },
} as const;

describe("compile/check control protocol v1", () => {
  it("selects the registered current authoring profile without a user codec input", () => {
    const request = studioCompileRequest("studio.current:1", "model.eqi", "model empty {}\n");
    expect(CURRENT_AUTHORING_MODEL_WIRE).toBe(currentProfile.modelWire);
    expect(request.modelWire).toBe(currentProfile.modelWire);
    expect(`eqiora.model-envelope/${request.modelWire}`).toBe(currentProfile.modelSchema);
  });

  it("shares the registered request fixtures with Rust and Python clients", () => {
    const accepted = compileRequestV1Schema.parse(acceptedFixture);
    const rejectedSource = compileRequestV1Schema.parse(rejectedSourceFixture);

    expect(accepted.requestId).toBe(expectedContract.accepted.requestId);
    expect(accepted.modelWire).toBe(expectedContract.accepted.modelWire);
    expect(rejectedSource.requestId).toBe(expectedContract.rejectedSource.requestId);
    expect(unsupportedProtocolFixture.requestId).toBe("shared-unsupported-protocol-v1");
    expect(compileRequestV1Schema.safeParse(unsupportedProtocolFixture).success).toBe(false);
  });

  it("is bound to the committed closed JSON Schema", () => {
    const requestSchema = COMPILE_V1_SCHEMA_DOCUMENT.$defs.request as {
      readonly additionalProperties: boolean;
      readonly required: readonly string[];
      readonly "x-eqiora-maxEncodedUtf8Bytes": number;
      readonly properties: {
        readonly filename: { readonly "x-eqiora-maxUtf8Bytes": number };
        readonly source: { readonly "x-eqiora-maxUtf8Bytes": number };
      };
    };
    const responseSchema = COMPILE_V1_SCHEMA_DOCUMENT.$defs.response as {
      readonly additionalProperties: boolean;
      readonly required: readonly string[];
    };
    const modelWireSchema = COMPILE_V1_SCHEMA_DOCUMENT.$defs.modelWire as {
      readonly enum: readonly string[];
    };
    const requiredFeaturesSchema = COMPILE_V1_SCHEMA_DOCUMENT.$defs.requiredFeatures as {
      readonly items: { readonly enum: readonly string[] };
    };

    expect(requestSchema.additionalProperties).toBe(false);
    expect(requestSchema["x-eqiora-maxEncodedUtf8Bytes"]).toBe(8 * 1_024 * 1_024 + 16 * 1_024);
    expect(requestSchema.properties.filename["x-eqiora-maxUtf8Bytes"]).toBe(4_096);
    expect(requestSchema.properties.source["x-eqiora-maxUtf8Bytes"]).toBe(8 * 1_024 * 1_024);
    expect(requestSchema.required).toEqual([
      "protocol",
      "command",
      "requestId",
      "requiredFeatures",
      "modelWire",
      "filename",
      "source",
    ]);
    expect(responseSchema.additionalProperties).toBe(false);
    expect(responseSchema.required).toEqual([
      "protocol",
      "command",
      "requestId",
      "requiredFeatures",
      "modelWire",
      "outcome",
    ]);
    expect(modelWireSchema.enum).toContain("v6");
    expect(requiredFeaturesSchema.items.enum).toContain("model-wire/v6");
    expect(modelWireSchema.enum).toContain("v7");
    expect(requiredFeaturesSchema.items.enum).toContain("model-wire/v7");
  });

  it("rejects unknown fields, protocols, and feature combinations", () => {
    expect(compileRequestV1Schema.safeParse({ ...REQUEST, extra: true }).success).toBe(false);
    expect(
      compileRequestV1Schema.safeParse({ ...REQUEST, protocol: "eqiora.control/v2" }).success,
    ).toBe(false);
    expect(
      compileRequestV1Schema.safeParse({
        ...REQUEST,
        requiredFeatures: [COMPILE_COMMAND_V1, "model-wire/v3"],
      }).success,
    ).toBe(false);
    expect(
      compileRequestV1Schema.safeParse({
        ...REQUEST,
        requiredFeatures: [
          `model-wire/${CURRENT_AUTHORING_MODEL_WIRE}`,
          COMPILE_COMMAND_V1,
          `model-wire/${CURRENT_AUTHORING_MODEL_WIRE}`,
        ],
      }).success,
    ).toBe(true);
    expect(
      compileRequestV1Schema.safeParse({ ...REQUEST, filename: `${"界".repeat(1_366)}x` }).success,
    ).toBe(false);
    expect(
      compileRequestV1Schema.safeParse({ ...REQUEST, filename: "bad\nname.eqi" }).success,
    ).toBe(false);
  });

  it("accepts only a closed response for the exact request identity", () => {
    const response = compileResponseV1Schema.parse(ACCEPTED);
    expect(compileResponseMatchesRequest(REQUEST, response)).toBe(true);
    expect(
      compileResponseMatchesRequest(REQUEST, { ...response, requestId: "studio.compile:8" }),
    ).toBe(false);
    expect(compileResponseV1Schema.safeParse({ ...ACCEPTED, source: "leak" }).success).toBe(false);

    const unordered = compileRequestV1Schema.parse({
      ...REQUEST,
      requiredFeatures: [
        `model-wire/${CURRENT_AUTHORING_MODEL_WIRE}`,
        COMPILE_COMMAND_V1,
        `model-wire/${CURRENT_AUTHORING_MODEL_WIRE}`,
      ],
    });
    expect(compileResponseMatchesRequest(unordered, response)).toBe(true);
  });

  it("preserves structured diagnostics without broadening the response", () => {
    const rejected = compileResponseV1Schema.parse({
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
  });
});
