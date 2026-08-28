import { invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import { checkedRequest, protocolFailure } from "./bridge-contract";
import {
  type CompileRequestV2,
  type ControlDiagnosticV2,
  compileRequestV2Schema,
  compileResponseMatchesRequest,
  compileResponseV2Schema,
} from "./control-protocol";
import type { DcMotorDemoRequest, DcMotorDemoResult } from "./dc-motor-demo-protocol";
import { nativeDemoBridge, previewDemoBridge } from "./demo-bridge";
import { CAD_EXAMPLE_SOURCE, CAD_PREVIEW_MODEL_DIGEST, EXAMPLE_SOURCE } from "./example";
import {
  BRIDGE_PROTOCOL,
  type BridgeEnvelope,
  bridgeEnvelopeSchema,
  type DocumentProjection,
  diagnosticSchema,
  documentProjectionSchema,
  type StudioDiagnostic,
} from "./protocol";
import {
  type ValueEditCommitRequest,
  type ValueEditPlan,
  type ValueEditPreviewRequest,
  type ValueEditResult,
  valueEditCommitRequestSchema,
  valueEditPlanSchema,
  valueEditPreviewRequestSchema,
  valueEditResultSchema,
} from "./value-edit-protocol";

export type BridgeMode = "native" | "preview";
export type StudioExample = "decay" | "cad";

export interface StudioBridge {
  readonly mode: BridgeMode;
  compile(request: CompileRequestV2): Promise<BridgeEnvelope<DocumentProjection>>;
  loadReadOnlyExample(
    example: StudioExample,
    request: CompileRequestV2,
  ): Promise<BridgeEnvelope<DocumentProjection>>;
  previewValueEdit(request: ValueEditPreviewRequest): Promise<BridgeEnvelope<ValueEditPlan>>;
  commitValueEdit(request: ValueEditCommitRequest): Promise<BridgeEnvelope<ValueEditResult>>;
  runDcMotorDemo(request: DcMotorDemoRequest): Promise<BridgeEnvelope<DcMotorDemoResult>>;
}

const compileCommandEnvelopeSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    control: compileResponseV2Schema.nullable(),
    projection: documentProjectionSchema.nullable(),
    diagnostics: z.array(diagnosticSchema).max(10_000),
  })
  .strict();

function studioDiagnostic(diagnostic: ControlDiagnosticV2): StudioDiagnostic {
  return {
    source: diagnostic.source,
    severity: diagnostic.severity,
    code: diagnostic.code,
    message: diagnostic.message,
    graphPath: diagnostic.graphPath,
    span: diagnostic.span,
    patch: diagnostic.patch,
  };
}

function exampleSource(example: StudioExample): string {
  switch (example) {
    case "decay":
      return EXAMPLE_SOURCE;
    case "cad":
      return CAD_EXAMPLE_SOURCE;
  }
}

function exampleRequestMatchesSource(example: StudioExample, request: CompileRequestV2): boolean {
  return request.source === exampleSource(example);
}

async function nativeCompile(
  request: CompileRequestV2,
): Promise<BridgeEnvelope<DocumentProjection>> {
  const checked = checkedRequest(compileRequestV2Schema, request, "Compile/check");
  if (!checked.ok) {
    return checked.failure;
  }
  let response: unknown;
  try {
    response = await invoke("compile_model", { requestJson: JSON.stringify(checked.value) });
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return protocolFailure(`Native bridge call compile_model failed: ${detail}`);
  }
  const decoded = compileCommandEnvelopeSchema.safeParse(response);
  if (!decoded.success) {
    return protocolFailure("Native bridge returned an invalid compile_model response.");
  }
  const envelope = decoded.data;
  if (envelope.control === null) {
    return envelope.projection === null && envelope.diagnostics.length > 0
      ? { protocol: BRIDGE_PROTOCOL, result: null, diagnostics: envelope.diagnostics }
      : protocolFailure("Native bridge returned an incoherent rejected compile/check response.");
  }
  if (!compileResponseMatchesRequest(checked.value, envelope.control)) {
    return protocolFailure("Native bridge returned compile/check metadata for another request.");
  }
  if (envelope.control.outcome.status === "rejected") {
    return envelope.projection === null && envelope.diagnostics.length === 0
      ? {
          protocol: BRIDGE_PROTOCOL,
          result: null,
          diagnostics: envelope.control.outcome.diagnostics.map(studioDiagnostic),
        }
      : protocolFailure("Native bridge mixed a rejected compile/check response with Studio state.");
  }
  if (envelope.projection === null) {
    return envelope.diagnostics.length > 0
      ? { protocol: BRIDGE_PROTOCOL, result: null, diagnostics: envelope.diagnostics }
      : protocolFailure("Native bridge omitted the accepted Model projection.");
  }
  const model = envelope.control.outcome.model;
  if (
    envelope.diagnostics.length > 0 ||
    envelope.projection.digest !== model.digest ||
    envelope.projection.modelId !== model.modelId ||
    envelope.projection.revision !== model.semanticRevision
  ) {
    return protocolFailure("Studio projection identity differs from the accepted canonical Model.");
  }
  return { protocol: BRIDGE_PROTOCOL, result: envelope.projection, diagnostics: [] };
}

async function checkedInvoke<T>(
  command: string,
  args: Record<string, unknown>,
  schema: ReturnType<typeof bridgeEnvelopeSchema>,
): Promise<BridgeEnvelope<T>> {
  try {
    const response: unknown = await invoke(command, args);
    const decoded = schema.safeParse(response);
    if (!decoded.success) {
      return protocolFailure(`Native bridge returned an invalid ${command} response.`);
    }
    return decoded.data as BridgeEnvelope<T>;
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return protocolFailure(`Native bridge call ${command} failed: ${detail}`);
  }
}

const nativeBridge: StudioBridge = {
  mode: "native",
  compile: nativeCompile,
  async loadReadOnlyExample(example, request) {
    return exampleRequestMatchesSource(example, request)
      ? nativeCompile(request)
      : protocolFailure("Read-only example identity does not match its immutable source.");
  },
  async previewValueEdit(request) {
    const checked = checkedRequest(valueEditPreviewRequestSchema, request, "Value edit preview");
    if (!checked.ok) {
      return checked.failure;
    }
    return checkedInvoke<ValueEditPlan>(
      "preview_value_edit",
      { request: checked.value },
      bridgeEnvelopeSchema(valueEditPlanSchema),
    );
  },
  async commitValueEdit(request) {
    const checked = checkedRequest(valueEditCommitRequestSchema, request, "Value edit commit");
    if (!checked.ok) {
      return checked.failure;
    }
    return checkedInvoke<ValueEditResult>(
      "commit_value_edit",
      { request: checked.value },
      bridgeEnvelopeSchema(valueEditResultSchema),
    );
  },
  async runDcMotorDemo(request) {
    return nativeDemoBridge.runDcMotor(request);
  },
};

const PREVIEW_DIGEST = "preview-4b6ec236856d4bf394168dbac7f5851b";

const previewDocument: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: PREVIEW_DIGEST,
  revision: 1,
  modelId: "Model:01J8EQIORASTUDIOPREVIEW000",
  nodes: [
    {
      id: "Field:state",
      name: "state",
      kind: "field",
      summary: "Scalar state with an initial value",
      dimension: "1",
      value: 1,
    },
    {
      id: "Parameter:rate",
      name: "rate",
      kind: "parameter",
      summary: "Canonical model parameter",
      dimension: "T^-1",
      value: 0.8,
    },
    {
      id: "Relation:decay",
      name: "decay",
      kind: "relation",
      summary: "1 implicit residual · 5 expression operations",
      dimension: null,
      value: null,
    },
    {
      id: "Activation:decay",
      name: "decay activation",
      kind: "activation",
      summary: "Continuous activation",
      dimension: null,
      value: null,
    },
  ],
  edges: [
    {
      id: "Relation:decay→Field:state:depends-on",
      source: "Relation:decay",
      target: "Field:state",
      kind: "depends-on",
      label: "depends on",
    },
    {
      id: "Relation:decay→Parameter:rate:depends-on",
      source: "Relation:decay",
      target: "Parameter:rate",
      kind: "depends-on",
      label: "depends on",
    },
    {
      id: "Activation:decay→Relation:decay:activates",
      source: "Activation:decay",
      target: "Relation:decay",
      kind: "activates",
      label: "activates",
    },
  ],
};

const previewCadDocument: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: CAD_PREVIEW_MODEL_DIGEST,
  revision: 1,
  modelId: "Model:01J8EQIORACADPREVIEW00000",
  nodes: [
    {
      id: "Domain:body",
      name: "body",
      kind: "domain",
      summary: "3D Cartesian body realized by the exact CAD plan",
      dimension: null,
      value: null,
    },
    ...(["x_lower", "x_upper", "y_lower", "y_upper", "z_lower", "z_upper"] as const).map(
      (name) => ({
        id: `Domain:${name}`,
        name,
        kind: "domain" as const,
        summary: "Semantic boundary retained independently of CAD face order",
        dimension: null,
        value: null,
      }),
    ),
    {
      id: "Representation:geometry_space",
      name: "geometry_space",
      kind: "representation",
      summary: "Continuous field representation",
      dimension: null,
      value: null,
    },
    {
      id: "Field:marker",
      name: "marker",
      kind: "field",
      summary: "Scalar field projected through the selected physical boundary",
      dimension: "1",
      value: 0,
    },
    {
      id: "Relation:selected_boundary",
      name: "selected_boundary",
      kind: "relation",
      summary: "Physical boundary relation on x_upper",
      dimension: null,
      value: null,
    },
  ],
  edges: [
    ...(["x_lower", "x_upper", "y_lower", "y_upper", "z_lower", "z_upper"] as const).map(
      (name) => ({
        id: `Domain:${name}→Domain:body:boundary-of`,
        source: `Domain:${name}`,
        target: "Domain:body",
        kind: "boundary-of",
        label: "boundary of",
      }),
    ),
    {
      id: "Field:marker→Domain:body:defined-on",
      source: "Field:marker",
      target: "Domain:body",
      kind: "defined-on",
      label: "defined on",
    },
    {
      id: "Field:marker→Representation:geometry_space:represented-by",
      source: "Field:marker",
      target: "Representation:geometry_space",
      kind: "represented-by",
      label: "represented by",
    },
    {
      id: "Relation:selected_boundary→Domain:x_upper:applies-on",
      source: "Relation:selected_boundary",
      target: "Domain:x_upper",
      kind: "applies-on",
      label: "applies on",
    },
  ],
};

const previewDocuments = new Map<string, DocumentProjection>([
  [previewDocument.digest, previewDocument],
]);
const MAX_PREVIEW_DOCUMENTS = 32;
const previewLineage: string[] = [previewDocument.digest];

function resetPreviewLineage(document: DocumentProjection) {
  previewDocuments.clear();
  previewDocuments.set(document.digest, document);
  previewLineage.splice(0, previewLineage.length, document.digest);
}

function retainPreviewChild(baseDigest: string, child: DocumentProjection): boolean {
  const baseIndex = previewLineage.indexOf(baseDigest);
  if (baseIndex < 0) return false;
  for (const abandoned of previewLineage.splice(baseIndex + 1)) {
    previewDocuments.delete(abandoned);
  }
  previewLineage.push(child.digest);
  previewDocuments.set(child.digest, child);
  while (previewLineage.length > MAX_PREVIEW_DOCUMENTS) {
    const oldest = previewLineage.shift();
    if (oldest !== undefined) previewDocuments.delete(oldest);
  }
  return true;
}

function previewFingerprint(input: string): string {
  let state = 2_166_136_261;
  for (const byte of new TextEncoder().encode(input)) {
    state ^= byte;
    state = Math.imul(state, 16_777_619) >>> 0;
  }
  return state.toString(16).padStart(8, "0").repeat(8);
}

function previewValuePlan(
  document: DocumentProjection,
  request: ValueEditPreviewRequest,
): ValueEditPlan | null {
  const node = document.nodes.find((candidate) => candidate.id === request.targetId);
  if (
    node === undefined ||
    !["field", "parameter"].includes(node.kind) ||
    node.value === null ||
    node.dimension === null ||
    node.value === request.value
  ) {
    return null;
  }
  const transactionDigest = previewFingerprint(
    `${request.digest}\0${request.targetId}\0${request.value.toString()}`,
  );
  return {
    protocol: BRIDGE_PROTOCOL,
    key: `eqiora.preview-value-edit-plan/v1:${transactionDigest}`,
    baseDigest: request.digest,
    baseRevision: document.revision,
    targetId: request.targetId,
    before: { value: node.value, dimension: node.dimension },
    after: { value: request.value, dimension: node.dimension },
    transactionDigest,
  };
}

const previewBridge: StudioBridge = {
  mode: "preview",
  async compile(request) {
    const checked = checkedRequest(compileRequestV2Schema, request, "Compile/check");
    if (!checked.ok) {
      return checked.failure;
    }
    await Promise.resolve();
    return {
      protocol: BRIDGE_PROTOCOL,
      result: null,
      diagnostics: [
        {
          source: "studio",
          severity: "error",
          code: "STPREVIEW",
          message:
            "Browser preview cannot compile source. Open a read-only example or launch the native shell for canonical diagnostics.",
          graphPath: null,
          span: null,
        },
      ],
    };
  },
  async loadReadOnlyExample(example, request) {
    const checked = checkedRequest(compileRequestV2Schema, request, "Read-only example");
    if (!checked.ok) {
      return checked.failure;
    }
    if (!exampleRequestMatchesSource(example, checked.value)) {
      return protocolFailure("Read-only example identity does not match its immutable source.");
    }
    await Promise.resolve();
    const document = example === "cad" ? previewCadDocument : previewDocument;
    resetPreviewLineage(document);
    return { protocol: BRIDGE_PROTOCOL, result: document, diagnostics: [] };
  },
  async previewValueEdit(request) {
    const checked = checkedRequest(valueEditPreviewRequestSchema, request, "Value edit preview");
    if (!checked.ok) {
      return checked.failure;
    }
    const document = previewDocuments.get(checked.value.digest);
    if (document === undefined) {
      return protocolFailure("Value-edit base revision is not available in the browser preview.");
    }
    const plan = previewValuePlan(document, checked.value);
    if (plan === null) {
      return protocolFailure("Select a quantitative entity and enter a different finite value.");
    }
    return { protocol: BRIDGE_PROTOCOL, result: plan, diagnostics: [] };
  },
  async commitValueEdit(request) {
    const checked = checkedRequest(valueEditCommitRequestSchema, request, "Value edit commit");
    if (!checked.ok) {
      return checked.failure;
    }
    const document = previewDocuments.get(checked.value.digest);
    if (document === undefined) {
      return protocolFailure("Value-edit base revision is not available in the browser preview.");
    }
    const plan = previewValuePlan(document, checked.value);
    if (plan === null || plan.key !== checked.value.planKey) {
      return protocolFailure("Value edit no longer matches the browser preview.");
    }
    const resultDigest = `preview-${previewFingerprint(`${plan.key}\0child`)}`;
    const child: DocumentProjection = {
      ...document,
      digest: resultDigest,
      revision: document.revision + 1,
      nodes: document.nodes.map((node) =>
        node.id === plan.targetId ? { ...node, value: plan.after.value } : node,
      ),
    };
    if (!retainPreviewChild(document.digest, child)) {
      return protocolFailure("Value-edit base revision left the browser preview lineage.");
    }
    return {
      protocol: BRIDGE_PROTOCOL,
      result: {
        protocol: BRIDGE_PROTOCOL,
        document: child,
        evidence: {
          plan,
          resultDigest,
          resultRevision: child.revision,
        },
      },
      diagnostics: [],
    };
  },
  async runDcMotorDemo(request) {
    return previewDemoBridge.runDcMotor(request);
  },
};

const hasTauriRuntime = "__TAURI_INTERNALS__" in window;

export const studioBridge = hasTauriRuntime ? nativeBridge : previewBridge;
