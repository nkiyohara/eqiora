import { Channel, invoke } from "@tauri-apps/api/core";
import { z } from "zod";
import {
  checkedRequest,
  outcomeMatchesRunRequest,
  protocolFailure,
  spatialResultMatchesRequest,
} from "./bridge-contract";
import {
  type CompileRequestV1,
  type ControlDiagnosticV1,
  compileRequestV1Schema,
  compileResponseMatchesRequest,
  compileResponseV1Schema,
} from "./control-protocol";
import {
  CAD_EXAMPLE_SOURCE,
  CAD_PREVIEW_MODEL_DIGEST,
  EXAMPLE_SOURCE,
  SPATIAL_EXAMPLE_SOURCE,
} from "./example";
import {
  BRIDGE_PROTOCOL,
  type BridgeEnvelope,
  bridgeEnvelopeSchema,
  type CancelRunRequest,
  type CancelRunResult,
  cancelRunRequestSchema,
  cancelRunResultSchema,
  type DocumentProjection,
  diagnosticSchema,
  documentProjectionSchema,
  MAX_SPATIAL_ENTITY_COUNT,
  type RunOutcome,
  type RunPlan,
  type RunPreviewRequest,
  type RunProgress,
  type RunRequest,
  runOutcomeSchema,
  runPlanSchema,
  runPreviewRequestSchema,
  runProgressSchema,
  runRequestSchema,
  type SpatialRealizationPreviewRequest,
  type SpatialRealizationRunRequest,
  type SpatialRunPlan,
  type SpatialRunResult,
  type StudioDiagnostic,
  spatialRealizationPreviewRequestSchema,
  spatialRealizationRunRequestSchema,
  spatialRunPlanSchema,
  spatialRunResultSchema,
  type ValueEditCommitRequest,
  type ValueEditPlan,
  type ValueEditPreviewRequest,
  type ValueEditResult,
  valueEditCommitRequestSchema,
  valueEditPlanSchema,
  valueEditPreviewRequestSchema,
  valueEditResultSchema,
} from "./protocol";

export type BridgeMode = "native" | "preview";
export type StudioExample = "decay" | "spatial" | "cad";

export interface StudioBridge {
  readonly mode: BridgeMode;
  compile(request: CompileRequestV1): Promise<BridgeEnvelope<DocumentProjection>>;
  loadReadOnlyExample(
    example: StudioExample,
    request: CompileRequestV1,
  ): Promise<BridgeEnvelope<DocumentProjection>>;
  previewValueEdit(request: ValueEditPreviewRequest): Promise<BridgeEnvelope<ValueEditPlan>>;
  commitValueEdit(request: ValueEditCommitRequest): Promise<BridgeEnvelope<ValueEditResult>>;
  previewRun(request: RunPreviewRequest): Promise<BridgeEnvelope<RunPlan>>;
  run(
    request: RunRequest,
    onProgress: (progress: RunProgress) => void,
  ): Promise<BridgeEnvelope<RunOutcome>>;
  cancelRun(request: CancelRunRequest): Promise<BridgeEnvelope<CancelRunResult>>;
  previewSpatialRealization(
    request: SpatialRealizationPreviewRequest,
  ): Promise<BridgeEnvelope<SpatialRunPlan>>;
  runSpatialRealization(
    request: SpatialRealizationRunRequest,
  ): Promise<BridgeEnvelope<SpatialRunResult>>;
}

const compileCommandEnvelopeSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    control: compileResponseV1Schema.nullable(),
    projection: documentProjectionSchema.nullable(),
    diagnostics: z.array(diagnosticSchema).max(10_000),
  })
  .strict();

function studioDiagnostic(diagnostic: ControlDiagnosticV1): StudioDiagnostic {
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
    case "spatial":
      return SPATIAL_EXAMPLE_SOURCE;
    case "cad":
      return CAD_EXAMPLE_SOURCE;
  }
}

function exampleRequestMatchesSource(example: StudioExample, request: CompileRequestV1): boolean {
  return request.source === exampleSource(example);
}

async function nativeCompile(
  request: CompileRequestV1,
): Promise<BridgeEnvelope<DocumentProjection>> {
  const checked = checkedRequest(compileRequestV1Schema, request, "Compile/check");
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
  async previewRun(request) {
    const checked = checkedRequest(runPreviewRequestSchema, request, "Run preview");
    if (!checked.ok) {
      return checked.failure;
    }
    return checkedInvoke<RunPlan>(
      "preview_reference_run",
      { request: checked.value },
      bridgeEnvelopeSchema(runPlanSchema),
    );
  },
  async run(request, onProgress) {
    const checked = checkedRequest(runRequestSchema, request, "Run");
    if (!checked.ok) {
      return checked.failure;
    }
    let invalidProgress = false;
    const progressChannel = new Channel<unknown>();
    progressChannel.onmessage = (message) => {
      const decoded = runProgressSchema.safeParse(message);
      if (!decoded.success || decoded.data.runId !== checked.value.runId) {
        invalidProgress = true;
        return;
      }
      onProgress(decoded.data);
    };
    const outcome = await checkedInvoke<RunOutcome>(
      "run_reference",
      { request: checked.value, onProgress: progressChannel },
      bridgeEnvelopeSchema(runOutcomeSchema),
    );
    if (invalidProgress) {
      return protocolFailure("Native bridge returned invalid or misrouted run progress.");
    }
    if (outcome.result !== null && !outcomeMatchesRunRequest(checked.value, outcome.result)) {
      return protocolFailure("Native bridge returned a run outcome for another request.");
    }
    return outcome;
  },
  async cancelRun(request) {
    const checked = checkedRequest(cancelRunRequestSchema, request, "Run cancellation");
    if (!checked.ok) {
      return checked.failure;
    }
    return checkedInvoke<CancelRunResult>(
      "cancel_reference_run",
      { request: checked.value },
      bridgeEnvelopeSchema(cancelRunResultSchema),
    );
  },
  async previewSpatialRealization(request) {
    const checked = checkedRequest(
      spatialRealizationPreviewRequestSchema,
      request,
      "Spatial Realization preview",
    );
    if (!checked.ok) {
      return checked.failure;
    }
    return checkedInvoke<SpatialRunPlan>(
      "preview_spatial_realization",
      { request: checked.value },
      bridgeEnvelopeSchema(spatialRunPlanSchema),
    );
  },
  async runSpatialRealization(request) {
    const checked = checkedRequest(
      spatialRealizationRunRequestSchema,
      request,
      "Spatial Realization run",
    );
    if (!checked.ok) {
      return checked.failure;
    }
    const response = await checkedInvoke<SpatialRunResult>(
      "run_spatial_realization",
      { request: checked.value },
      bridgeEnvelopeSchema(spatialRunResultSchema),
    );
    if (response.result !== null && !spatialResultMatchesRequest(checked.value, response.result)) {
      return protocolFailure("Native bridge returned a spatial result for another request.");
    }
    return response;
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
  workflows: { scalarElliptic: null },
};

const PREVIEW_SPATIAL_DIGEST = "preview-a91d4fc5de804ff5b64b141a954c4d37";

const previewSpatialDocument: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: PREVIEW_SPATIAL_DIGEST,
  revision: 1,
  modelId: "Model:01J8EQIORASPATIALPREVIEW00",
  nodes: [
    {
      id: "Domain:square",
      name: "square",
      kind: "domain",
      summary: "2D Cartesian continuous domain",
      dimension: null,
      value: null,
    },
    {
      id: "Representation:scalar_space",
      name: "scalar_space",
      kind: "representation",
      summary: "Continuous field representation",
      dimension: null,
      value: null,
    },
    {
      id: "Field:potential",
      name: "potential",
      kind: "field",
      summary: "Scalar field with an initial value",
      dimension: "1",
      value: 0,
    },
    {
      id: "Parameter:wave_number",
      name: "wave_number",
      kind: "parameter",
      summary: "Canonical model parameter",
      dimension: "L^-1",
      value: Math.PI,
    },
    {
      id: "Parameter:source_scale",
      name: "source_scale",
      kind: "parameter",
      summary: "Canonical model parameter",
      dimension: "L^-2",
      value: 2 * Math.PI * Math.PI,
    },
    {
      id: "Relation:balance",
      name: "balance",
      kind: "relation",
      summary: "1 implicit spatial residual · scalar elliptic form",
      dimension: null,
      value: null,
    },
  ],
  edges: [
    {
      id: "Field:potential→Domain:square:defined-on",
      source: "Field:potential",
      target: "Domain:square",
      kind: "defined-on",
      label: "defined on",
    },
    {
      id: "Field:potential→Representation:scalar_space:represented-by",
      source: "Field:potential",
      target: "Representation:scalar_space",
      kind: "represented-by",
      label: "represented by",
    },
    {
      id: "Relation:balance→Field:potential:depends-on",
      source: "Relation:balance",
      target: "Field:potential",
      kind: "depends-on",
      label: "depends on",
    },
    {
      id: "Relation:balance→Parameter:wave_number:depends-on",
      source: "Relation:balance",
      target: "Parameter:wave_number",
      kind: "depends-on",
      label: "depends on",
    },
    {
      id: "Relation:balance→Parameter:source_scale:depends-on",
      source: "Relation:balance",
      target: "Parameter:source_scale",
      kind: "depends-on",
      label: "depends on",
    },
  ],
  workflows: {
    scalarElliptic: {
      spatialDimension: 2,
      scalarType: "f64",
      vectorLayout: "replicated",
      maximumHostWorkers: 8,
      workerBudgetSource: "studio-session-budget",
    },
  },
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
  workflows: { scalarElliptic: null },
};

const previewDocuments = new Map<string, DocumentProjection>([
  [previewDocument.digest, previewDocument],
]);
const MAX_PREVIEW_DOCUMENTS = 32;
const PREVIEW_PROGRESS_INTERVAL_MS = 100;
const previewLineage: string[] = [previewDocument.digest];
const previewCancellations = new Map<string, { cancelled: boolean }>();
let previewSpatialRunId: string | null = null;

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

function previewPlan(request: RunPreviewRequest): RunPlan {
  return {
    protocol: BRIDGE_PROTOCOL,
    key: `eqiora.preview-reference-plan/v1:${request.endTime}:${request.maxStep}`,
    adapter: { id: "eqiora.preview-reference", version: "0.1.0" },
    placement: { kind: "host", workers: 1 },
    integration: {
      method: "backward-euler",
      endTime: request.endTime,
      maxStep: request.maxStep,
    },
    nonlinear: {
      method: "dense-finite-difference-newton",
      absoluteTolerance: 1e-10,
      relativeTolerance: 1e-10,
      maximumIterations: 32,
    },
    events: {
      timeTolerance: 1e-10,
      guardTolerance: 1e-10,
      maximumLocalizationIterations: 80,
      maximumZeroTimeEvents: 64,
    },
    limits: { maximumSteps: 1_000_000 },
    acceptance: { kind: "semantic-oracle", independentVerifier: false },
  };
}

function previewSpatialPlan(request: SpatialRealizationPreviewRequest): SpatialRunPlan | null {
  const workflow = previewDocuments.get(request.digest)?.workflows.scalarElliptic;
  if (
    workflow === undefined ||
    workflow === null ||
    request.workers > workflow.maximumHostWorkers
  ) {
    return null;
  }
  const cellCount = request.cellsPerAxis ** workflow.spatialDimension;
  const fieldAxis =
    request.method === "finite-element" ? request.cellsPerAxis + 1 : request.cellsPerAxis;
  const fieldValueCount = fieldAxis ** workflow.spatialDimension;
  if (
    !Number.isSafeInteger(cellCount) ||
    !Number.isSafeInteger(fieldValueCount) ||
    cellCount > MAX_SPATIAL_ENTITY_COUNT ||
    fieldValueCount > MAX_SPATIAL_ENTITY_COUNT
  ) {
    return null;
  }
  const key = previewFingerprint(
    `${request.digest}\0${request.realizationRevision}\0${request.method}\0${request.cellsPerAxis}\0${request.workers}`,
  );
  const finiteElement = request.method === "finite-element";
  return {
    protocol: BRIDGE_PROTOCOL,
    key,
    modelDigest: request.digest,
    realizationRevision: request.realizationRevision,
    requirements: {
      spatialDimension: workflow.spatialDimension,
      scalarType: "f64",
      vectorLayout: "replicated",
    },
    discretization: {
      method: request.method,
      space: finiteElement ? "continuous-lagrange" : "cell-constant",
      order: finiteElement ? 1 : null,
      mesh: "generated-cartesian",
      cellsPerAxis: request.cellsPerAxis,
      cellCount,
      quadrature: finiteElement ? "gauss-legendre" : "cell-centroid",
      pointsPerAxis: finiteElement ? 2 : null,
      fieldValueCount,
    },
    solver: {
      adapter: "eqiora.reference",
      algorithm: "conjugate-gradient",
      preconditioner: "identity",
      reduction: "reproducible",
      relativeTolerance: 1e-10,
      absoluteTolerance: 1e-12,
      maximumIterations: 10_000,
    },
    placement: {
      kind: "host",
      adapter: request.workers === 1 ? "eqiora.host.serial" : "eqiora.rayon",
      workers: request.workers,
      maximumWorkers: workflow.maximumHostWorkers,
      budgetSource: "studio-session-budget",
    },
    limits: { maximumEntityCount: MAX_SPATIAL_ENTITY_COUNT },
    acceptance: {
      algebraic: "independent-true-residual",
      continuous: "boundary-source-balance",
      independentTrueResidual: true,
    },
  };
}

const previewBridge: StudioBridge = {
  mode: "preview",
  async compile(request) {
    const checked = checkedRequest(compileRequestV1Schema, request, "Compile/check");
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
    const checked = checkedRequest(compileRequestV1Schema, request, "Read-only example");
    if (!checked.ok) {
      return checked.failure;
    }
    if (!exampleRequestMatchesSource(example, checked.value)) {
      return protocolFailure("Read-only example identity does not match its immutable source.");
    }
    await Promise.resolve();
    const document =
      example === "spatial"
        ? previewSpatialDocument
        : example === "cad"
          ? previewCadDocument
          : previewDocument;
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
  async previewRun(request) {
    const checked = checkedRequest(runPreviewRequestSchema, request, "Run preview");
    if (!checked.ok) {
      return checked.failure;
    }
    return {
      protocol: BRIDGE_PROTOCOL,
      result: previewPlan(checked.value),
      diagnostics: [],
    };
  },
  async previewSpatialRealization(request) {
    const checked = checkedRequest(
      spatialRealizationPreviewRequestSchema,
      request,
      "Spatial Realization preview",
    );
    if (!checked.ok) {
      return checked.failure;
    }
    const plan = previewSpatialPlan(checked.value);
    return plan === null
      ? protocolFailure(
          "Spatial Realization is unsupported or exceeds the browser-preview resource budget.",
        )
      : { protocol: BRIDGE_PROTOCOL, result: plan, diagnostics: [] };
  },
  async run(request, onProgress) {
    const checked = checkedRequest(runRequestSchema, request, "Run");
    if (!checked.ok) {
      return checked.failure;
    }
    const plan = previewPlan(checked.value);
    if (checked.value.planKey !== plan.key) {
      return protocolFailure("Run plan no longer matches the browser preview.");
    }
    if (previewCancellations.size > 0 || previewSpatialRunId !== null) {
      return protocolFailure("Another browser-preview run is already active.");
    }
    const cancellation = { cancelled: false };
    previewCancellations.set(checked.value.runId, cancellation);
    const steps = Math.ceil(checked.value.endTime / checked.value.maxStep);
    const started = performance.now();
    try {
      const checkpoints = Array.from(
        new Set(Array.from({ length: 10 }, (_, index) => Math.floor(steps * (index / 10)))),
      );
      for (const acceptedSteps of checkpoints) {
        await new Promise<void>((resolve) =>
          window.setTimeout(resolve, PREVIEW_PROGRESS_INTERVAL_MS),
        );
        const modelTime = Math.min(acceptedSteps * checked.value.maxStep, checked.value.endTime);
        const progress: RunProgress = {
          protocol: BRIDGE_PROTOCOL,
          runId: checked.value.runId,
          modelTime,
          endTime: checked.value.endTime,
          acceptedSteps,
          maximumSteps: plan.limits.maximumSteps,
          elapsedSeconds: (performance.now() - started) / 1_000,
        };
        onProgress(progress);
        if (cancellation.cancelled) {
          return {
            protocol: BRIDGE_PROTOCOL,
            result: {
              kind: "cancelled" as const,
              cancellation: {
                protocol: BRIDGE_PROTOCOL,
                runId: checked.value.runId,
                plan,
                elapsedSeconds: progress.elapsedSeconds,
                progress,
              },
            },
            diagnostics: [],
          };
        }
      }

      const time = Array.from({ length: steps + 1 }, (_, index) =>
        Math.min(index * checked.value.maxStep, checked.value.endTime),
      );
      const document = previewDocuments.get(checked.value.digest);
      const rate =
        document?.nodes.find((node) => node.kind === "parameter" && node.name === "rate")?.value ??
        0.8;
      const values = time.map((value) => Math.exp(-rate * value));
      return {
        protocol: BRIDGE_PROTOCOL,
        result: {
          kind: "completed" as const,
          result: {
            protocol: BRIDGE_PROTOCOL,
            digest: checked.value.digest,
            evidence: {
              plan,
              elapsedSeconds: (performance.now() - started) / 1_000,
              fieldCount: 1,
              sampleCount: time.length,
            },
            series: [
              {
                fieldId: "Field:state",
                name: "state",
                dimension: "1",
                time,
                values,
              },
            ],
          },
        },
        diagnostics: [],
      };
    } finally {
      previewCancellations.delete(checked.value.runId);
    }
  },
  async runSpatialRealization(request) {
    const checked = checkedRequest(
      spatialRealizationRunRequestSchema,
      request,
      "Spatial Realization run",
    );
    if (!checked.ok) {
      return checked.failure;
    }
    const plan = previewSpatialPlan(checked.value);
    if (plan === null || plan.key !== checked.value.planKey) {
      return protocolFailure("Spatial run no longer matches the browser-preview Realization.");
    }
    if (previewCancellations.size > 0 || previewSpatialRunId !== null) {
      return protocolFailure("Another browser-preview run is already active.");
    }
    previewSpatialRunId = checked.value.runId;
    try {
      await new Promise<void>((resolve) => window.setTimeout(resolve, 180));
      const execution = {
        adapter: plan.placement.adapter,
        topology: { kind: "host" as const, workers: plan.placement.workers },
      };
      const result: SpatialRunResult = {
        protocol: BRIDGE_PROTOCOL,
        runId: checked.value.runId,
        digest: checked.value.digest,
        plan,
        elapsedSeconds: 0.18,
        field: {
          location: plan.discretization.method === "finite-element" ? "vertex" : "cell-center",
          valueCount: plan.discretization.fieldValueCount,
          minimum: 0,
          maximum: 1,
        },
        balance: {
          boundaryTotal: -8,
          integratedSource: 8,
          relativeImbalance: 2.1e-15,
        },
        assembly: {
          execution,
          packetCount: plan.discretization.cellCount,
          targetCount: plan.discretization.fieldValueCount,
        },
        solve: {
          backend: "eqiora.reference",
          execution,
          verification: execution,
          algorithm: "conjugate-gradient",
          preconditioner: "identity",
          reduction: "reproducible",
          reason: "residual-tolerance-satisfied",
          completedIterations: 24,
          initialResidualNorm: 1,
          reportedResidualNorm: 4.8e-12,
          trueResidualNorm: 5.1e-12,
          residualTarget: 1e-10,
        },
      };
      return spatialResultMatchesRequest(checked.value, result)
        ? { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] }
        : protocolFailure("Browser preview produced a mismatched spatial result.");
    } finally {
      previewSpatialRunId = null;
    }
  },
  async cancelRun(request) {
    const checked = checkedRequest(cancelRunRequestSchema, request, "Run cancellation");
    if (!checked.ok) {
      return checked.failure;
    }
    if (previewSpatialRunId === checked.value.runId) {
      return {
        protocol: BRIDGE_PROTOCOL,
        result: {
          protocol: BRIDGE_PROTOCOL,
          runId: checked.value.runId,
          status: "not-cancellable" as const,
        },
        diagnostics: [],
      };
    }
    const cancellation = previewCancellations.get(checked.value.runId);
    if (cancellation !== undefined) cancellation.cancelled = true;
    return {
      protocol: BRIDGE_PROTOCOL,
      result: {
        protocol: BRIDGE_PROTOCOL,
        runId: checked.value.runId,
        status: cancellation === undefined ? "already-terminal" : "requested",
      },
      diagnostics: [],
    };
  },
};

const hasTauriRuntime = "__TAURI_INTERNALS__" in window;

export const studioBridge = hasTauriRuntime ? nativeBridge : previewBridge;
