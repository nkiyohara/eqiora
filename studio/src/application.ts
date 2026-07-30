import { COMMAND_REGISTRY, WORKFLOW_REGISTRY } from "./application-registry";
import type {
  CAD_V1_SEMANTIC_ENTITY_COUNT,
  CAD_V1_TRIANGLE_COUNT,
  CAD_V1_VERTEX_COUNT,
} from "./cad-protocol";
import type { MessageKey } from "./messages";
import type {
  DocumentProjection,
  MAX_PROJECTION_EDGE_COUNT,
  MAX_PROJECTION_NODE_COUNT,
} from "./protocol";
import type { SCALAR_FIELD_VALUES_PER_CHUNK } from "./scalar-field-protocol";
import type { MAX_SPATIAL_ENTITY_COUNT } from "./spatial-protocol";
import type {
  UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
  UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT,
} from "./unstructured-field-protocol";

export { COMMAND_REGISTRY, WORKFLOW_REGISTRY } from "./application-registry";

export type WorkflowId =
  | "relations"
  | "scalar-elliptic"
  | "cylinder-stokes"
  | "packaged-dc-drive"
  | "structural-elasticity"
  | "fixed-reference-fsi"
  | "cad-box";
export type WorkspaceId = "relations" | "field" | "trajectory" | "structure" | "fsi" | "geometry";

export type CommandId =
  | "model.compile"
  | "edit.commit"
  | "history.undo"
  | "history.redo"
  | "run.execute"
  | "run.cancel"
  | "view.reflow"
  | "workspace.relations"
  | "workspace.field"
  | "workspace.trajectory"
  | "workspace.structure"
  | "workspace.fsi"
  | "workspace.geometry"
  | "example.cylinder"
  | "example.dc-drive"
  | "example.structural"
  | "example.fsi"
  | "example.spatial"
  | "example.cad"
  | "focus.source"
  | "focus.relation"
  | "focus.inspector"
  | "focus.evidence";

export type CommandGroup = "model" | "execution" | "view" | "navigate";

export type FocusTarget =
  | "source-editor"
  | "relation-view"
  | "selection-inspector"
  | "evidence-inspector"
  | "field-viewport"
  | "field-value-table"
  | "trajectory-viewport"
  | "trajectory-sample-table"
  | "structural-viewport"
  | "structural-vertex-table"
  | "fsi-viewport"
  | "fsi-vertex-table"
  | "cad-viewport"
  | "cad-domain-table";

export type ElementFocusTarget = Exclude<FocusTarget, "source-editor" | "relation-view">;

export const CYLINDER_EVIDENCE_FOCUS_ID = "cylinder-evidence-inspector";
export const DC_MOTOR_EVIDENCE_FOCUS_ID = "dc-drive-evidence-inspector";
export const STRUCTURAL_EVIDENCE_FOCUS_ID = "structural-evidence-inspector";
export const FSI_EVIDENCE_FOCUS_ID = "fsi-evidence-inspector";

/**
 * Resolve registry focus to the one visible workflow-owned element.
 *
 * Workspaces remain mounted while hidden, so shared IDs or document-order
 * lookup would silently target an inactive surface.
 */
export function resolveElementFocusId(
  target: ElementFocusTarget,
  activeWorkflow: WorkflowId,
  activeWorkspace: WorkspaceId,
): string {
  switch (target) {
    case "selection-inspector":
      return activeWorkflow === "cad-box"
        ? "cad-selection-inspector"
        : activeWorkflow === "cylinder-stokes"
          ? "unstructured-field-inspector"
          : activeWorkflow === "packaged-dc-drive"
            ? "trajectory-sample-table"
            : activeWorkspace === "field"
              ? "field-selection-inspector"
              : "inspector-panel";
    case "evidence-inspector":
      return activeWorkflow === "cylinder-stokes"
        ? CYLINDER_EVIDENCE_FOCUS_ID
        : activeWorkflow === "packaged-dc-drive"
          ? DC_MOTOR_EVIDENCE_FOCUS_ID
          : activeWorkflow === "structural-elasticity"
            ? STRUCTURAL_EVIDENCE_FOCUS_ID
            : activeWorkflow === "fixed-reference-fsi"
              ? FSI_EVIDENCE_FOCUS_ID
              : "evidence-inspector";
    case "trajectory-viewport":
      return "trajectory-viewport";
    case "trajectory-sample-table":
      return "trajectory-sample-table";
    case "structural-viewport":
      return "structural-viewport";
    case "structural-vertex-table":
      return "structural-vertex-table";
    case "fsi-viewport":
      return "fsi-viewport";
    case "fsi-vertex-table":
      return "fsi-vertex-table";
    case "cad-viewport":
      return "cad-viewport";
    case "cad-domain-table":
      return "cad-domain-table";
    case "field-viewport":
      return activeWorkflow === "cylinder-stokes"
        ? "unstructured-field-viewport"
        : "field-viewport";
    case "field-value-table":
      return activeWorkflow === "cylinder-stokes"
        ? "unstructured-vertex-table"
        : "field-value-table";
  }
}

export interface CommandDefinition {
  readonly id: CommandId;
  readonly group: CommandGroup;
  readonly label: MessageKey;
  readonly description: MessageKey;
  readonly shortcut: string | null;
  readonly focusTarget: FocusTarget | null;
  readonly workflows: readonly WorkflowId[];
}

export type SemanticAlternative =
  | Readonly<{
      kind: "semantic-outline";
      focusTarget: "source-editor";
    }>
  | Readonly<{
      kind: "field-value-table";
      focusTarget: "field-value-table";
    }>
  | Readonly<{
      kind: "domain-table";
      focusTarget: "cad-domain-table";
    }>
  | Readonly<{
      kind: "trajectory-sample-table";
      focusTarget: "trajectory-sample-table";
    }>
  | Readonly<{
      kind: "structural-vertex-table";
      focusTarget: "structural-vertex-table";
    }>
  | Readonly<{
      kind: "fsi-vertex-table";
      focusTarget: "fsi-vertex-table";
    }>;

export type ProjectionContract =
  | Readonly<{
      kind: "semantic-relation-graph";
      maximumNodes: typeof MAX_PROJECTION_NODE_COUNT;
      maximumEdges: typeof MAX_PROJECTION_EDGE_COUNT;
      semanticAlternative: Extract<SemanticAlternative, { kind: "semantic-outline" }>;
    }>
  | Readonly<{
      kind: "bounded-scalar-field-view";
      maximumFieldValues: typeof MAX_SPATIAL_ENTITY_COUNT;
      transfer: "explicit-owned-host-copy";
      valuesPerChunk: typeof SCALAR_FIELD_VALUES_PER_CHUNK;
      semanticAlternative: Extract<SemanticAlternative, { kind: "field-value-table" }>;
    }>
  | Readonly<{
      kind: "bounded-unstructured-p1-field-view";
      maximumVertices: typeof MAX_SPATIAL_ENTITY_COUNT;
      maximumTriangles: typeof UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT;
      transfer: "explicit-owned-host-copy";
      itemsPerChunk: typeof UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK;
      semanticAlternative: Extract<SemanticAlternative, { kind: "field-value-table" }>;
    }>
  | Readonly<{
      kind: "bounded-cad-triangle-view";
      maximumVertices: typeof CAD_V1_VERTEX_COUNT;
      maximumTriangles: typeof CAD_V1_TRIANGLE_COUNT;
      maximumEntities: typeof CAD_V1_SEMANTIC_ENTITY_COUNT;
      semanticAlternative: Extract<SemanticAlternative, { kind: "domain-table" }>;
    }>
  | Readonly<{
      kind: "bounded-sampled-trajectory-view";
      maximumSamples: 101;
      maximumCommits: 11;
      semanticAlternative: Extract<SemanticAlternative, { kind: "trajectory-sample-table" }>;
    }>
  | Readonly<{
      kind: "bounded-cartesian-displacement-grid-view";
      maximumVertices: 289;
      maximumCells: 256;
      components: 2;
      semanticAlternative: Extract<SemanticAlternative, { kind: "structural-vertex-table" }>;
    }>
  | Readonly<{
      kind: "bounded-fixed-reference-fsi-trajectory-view";
      maximumVertices: 9;
      maximumTriangles: 8;
      acceptedSteps: 2;
      components: 2;
      semanticAlternative: Extract<SemanticAlternative, { kind: "fsi-vertex-table" }>;
    }>;

type RelationsWorkflowDefinition = Readonly<{
  id: "relations";
  workspace: "relations";
  label: "workflow.relations.label";
  description: "workflow.relations.description";
  primaryFocus: "relation-view";
  projection: Extract<ProjectionContract, { kind: "semantic-relation-graph" }>;
}>;

type ScalarEllipticWorkflowDefinition = Readonly<{
  id: "scalar-elliptic";
  workspace: "relations";
  label: "workflow.spatial.label";
  description: "workflow.spatial.description";
  primaryFocus: "relation-view";
  projection: Extract<ProjectionContract, { kind: "bounded-scalar-field-view" }>;
}>;

type CylinderWorkflowDefinition = Readonly<{
  id: "cylinder-stokes";
  workspace: "field";
  label: "workflow.cylinder.label";
  description: "workflow.cylinder.description";
  primaryFocus: "field-viewport";
  projection: Extract<ProjectionContract, { kind: "bounded-unstructured-p1-field-view" }>;
}>;

type DcMotorWorkflowDefinition = Readonly<{
  id: "packaged-dc-drive";
  workspace: "trajectory";
  label: "workflow.dc-drive.label";
  description: "workflow.dc-drive.description";
  primaryFocus: "trajectory-viewport";
  projection: Extract<ProjectionContract, { kind: "bounded-sampled-trajectory-view" }>;
}>;

type StructuralWorkflowDefinition = Readonly<{
  id: "structural-elasticity";
  workspace: "structure";
  label: "workflow.structural.label";
  description: "workflow.structural.description";
  primaryFocus: "structural-viewport";
  projection: Extract<ProjectionContract, { kind: "bounded-cartesian-displacement-grid-view" }>;
}>;

type FsiWorkflowDefinition = Readonly<{
  id: "fixed-reference-fsi";
  workspace: "fsi";
  label: "workflow.fsi.label";
  description: "workflow.fsi.description";
  primaryFocus: "fsi-viewport";
  projection: Extract<ProjectionContract, { kind: "bounded-fixed-reference-fsi-trajectory-view" }>;
}>;

type CadWorkflowDefinition = Readonly<{
  id: "cad-box";
  workspace: "geometry";
  label: "workflow.cad.label";
  description: "workflow.cad.description";
  primaryFocus: "cad-viewport";
  projection: Extract<ProjectionContract, { kind: "bounded-cad-triangle-view" }>;
}>;

export type WorkflowDefinition =
  | RelationsWorkflowDefinition
  | ScalarEllipticWorkflowDefinition
  | CylinderWorkflowDefinition
  | DcMotorWorkflowDefinition
  | StructuralWorkflowDefinition
  | FsiWorkflowDefinition
  | CadWorkflowDefinition;

export type AcceptedApplicationProjection = Pick<DocumentProjection, "digest" | "workflows">;

export type CadApplicationStatus = "idle" | "loading" | "ready" | "unavailable";
export type CylinderApplicationStatus = "idle" | "running" | "ready" | "failed";
export type DcMotorApplicationStatus = "idle" | "running" | "ready" | "failed";
export type StructuralApplicationStatus = "idle" | "running" | "ready" | "failed";
export type FsiApplicationStatus = "idle" | "running" | "ready" | "failed";

export type CadApplicationInput = Readonly<{
  status: CadApplicationStatus;
  /**
   * Digest carried by an accepted CAD projection. A status value alone cannot
   * make another Model's projection applicable.
   */
  acceptedModelDigest: string | null;
}>;

export type ApplicationInputs = Readonly<{
  acceptedProjection: AcceptedApplicationProjection | null;
  cad: CadApplicationInput;
  cylinderStatus: CylinderApplicationStatus;
  dcMotorStatus: DcMotorApplicationStatus;
  structuralStatus: StructuralApplicationStatus;
  fsiStatus: FsiApplicationStatus;
  fieldWorkflow: "scalar-elliptic" | "cylinder-stokes" | null;
}>;

export type WorkflowAvailability =
  | Readonly<{ kind: "available"; reason: null }>
  | Readonly<{ kind: "loading"; reason: MessageKey }>
  | Readonly<{ kind: "unavailable"; reason: MessageKey }>;

export type ResolvedWorkflow = Readonly<{
  definition: WorkflowDefinition;
  availability: WorkflowAvailability;
  commands: readonly CommandDefinition[];
}>;

function unavailable(reason: MessageKey): WorkflowAvailability {
  return { kind: "unavailable", reason };
}

function resolveAvailability(
  workflow: WorkflowDefinition,
  inputs: ApplicationInputs,
): WorkflowAvailability {
  if (workflow.id === "cylinder-stokes") {
    switch (inputs.cylinderStatus) {
      case "running":
        return { kind: "loading", reason: "workflow.reason.cylinder-running" };
      case "ready":
        return { kind: "available", reason: null };
      case "idle":
      case "failed":
        return unavailable("workflow.reason.cylinder-unavailable");
    }
  }
  if (workflow.id === "packaged-dc-drive") {
    switch (inputs.dcMotorStatus) {
      case "running":
        return { kind: "loading", reason: "workflow.reason.dc-drive-running" };
      case "ready":
        return { kind: "available", reason: null };
      case "idle":
      case "failed":
        return unavailable("workflow.reason.dc-drive-unavailable");
    }
  }
  if (workflow.id === "structural-elasticity") {
    switch (inputs.structuralStatus) {
      case "running":
        return { kind: "loading", reason: "workflow.reason.structural-running" };
      case "ready":
        return { kind: "available", reason: null };
      case "idle":
      case "failed":
        return unavailable("workflow.reason.structural-unavailable");
    }
  }
  if (workflow.id === "fixed-reference-fsi") {
    switch (inputs.fsiStatus) {
      case "running":
        return { kind: "loading", reason: "workflow.reason.fsi-running" };
      case "ready":
        return { kind: "available", reason: null };
      case "idle":
      case "failed":
        return unavailable("workflow.reason.fsi-unavailable");
    }
  }
  const document = inputs.acceptedProjection;
  if (document === null) {
    return unavailable("workflow.reason.compile-first");
  }
  switch (workflow.id) {
    case "relations":
      return { kind: "available", reason: null };
    case "scalar-elliptic":
      return document.workflows.scalarElliptic === null
        ? unavailable("workflow.reason.spatial-unavailable")
        : { kind: "available", reason: null };
    case "cad-box":
      switch (inputs.cad.status) {
        case "idle":
        case "loading":
          return { kind: "loading", reason: "workflow.reason.cad-loading" };
        case "unavailable":
          return unavailable("workflow.reason.cad-unavailable");
        case "ready":
          return inputs.cad.acceptedModelDigest === document.digest
            ? { kind: "available", reason: null }
            : unavailable("workflow.reason.cad-stale");
      }
      throw new Error("Closed CAD status registry was not exhaustive");
  }
}

export function resolveApplicationWorkflows(
  inputs: ApplicationInputs,
): readonly ResolvedWorkflow[] {
  return WORKFLOW_REGISTRY.map((definition) => ({
    definition,
    availability: resolveAvailability(definition, inputs),
    commands: COMMAND_REGISTRY.filter((command) =>
      (command.workflows as readonly WorkflowId[]).includes(definition.id),
    ),
  }));
}

export type ResolvedApplication = Readonly<{
  requestedWorkspace: WorkspaceId;
  workspace: WorkspaceId;
  activeWorkflow: WorkflowId;
  workflows: readonly ResolvedWorkflow[];
  fellBack: boolean;
}>;

/**
 * Resolve presentation state without changing canonical or run state.
 *
 * Geometry remains visible while its exact projection is loading. If that
 * projection becomes unavailable or stale, the shell returns to Relations
 * instead of leaving an inapplicable workspace active.
 */
export function resolveApplication(
  inputs: ApplicationInputs,
  requestedWorkspace: WorkspaceId,
): ResolvedApplication {
  const workflows = resolveApplicationWorkflows(inputs);
  const cad = workflows.find((workflow) => workflow.definition.id === "cad-box");
  const geometryAvailable =
    cad?.availability.kind === "available" || cad?.availability.kind === "loading";
  const field = workflows.find((workflow) => workflow.definition.id === inputs.fieldWorkflow);
  const fieldAvailable =
    field?.availability.kind === "available" || field?.availability.kind === "loading";
  const trajectory = workflows.find((workflow) => workflow.definition.id === "packaged-dc-drive");
  const trajectoryAvailable =
    trajectory?.availability.kind === "available" || trajectory?.availability.kind === "loading";
  const structural = workflows.find(
    (workflow) => workflow.definition.id === "structural-elasticity",
  );
  const structuralAvailable =
    structural?.availability.kind === "available" || structural?.availability.kind === "loading";
  const fsi = workflows.find((workflow) => workflow.definition.id === "fixed-reference-fsi");
  const fsiAvailable =
    fsi?.availability.kind === "available" || fsi?.availability.kind === "loading";
  const workspace =
    (requestedWorkspace === "geometry" && !geometryAvailable) ||
    (requestedWorkspace === "field" && !fieldAvailable) ||
    (requestedWorkspace === "trajectory" && !trajectoryAvailable) ||
    (requestedWorkspace === "structure" && !structuralAvailable) ||
    (requestedWorkspace === "fsi" && !fsiAvailable)
      ? "relations"
      : requestedWorkspace;
  const spatial = workflows.find((workflow) => workflow.definition.id === "scalar-elliptic");
  const activeWorkflow: WorkflowId =
    workspace === "geometry"
      ? "cad-box"
      : workspace === "field"
        ? (inputs.fieldWorkflow ?? "relations")
        : workspace === "trajectory"
          ? "packaged-dc-drive"
          : workspace === "structure"
            ? "structural-elasticity"
            : workspace === "fsi"
              ? "fixed-reference-fsi"
              : spatial?.availability.kind === "available"
                ? "scalar-elliptic"
                : "relations";
  return {
    requestedWorkspace,
    workspace,
    activeWorkflow,
    workflows,
    fellBack: workspace !== requestedWorkspace,
  };
}

export type RunBlock =
  | "active-run"
  | "committing"
  | "no-document"
  | "source-edited"
  | "invalid-spatial"
  | "spatial-plan"
  | "invalid-time"
  | "time-plan"
  | null;

export type RunActivity = "idle" | "reference-running" | "reference-cancelling" | "spatial-running";

export type ValueEditBlock = "run" | "source" | null;

export type CommandFacts = Readonly<{
  activeWorkflow: WorkflowId;
  compiling: boolean;
  documentAccepted: boolean;
  valueEditReady: boolean;
  valueEditBlock: ValueEditBlock;
  revisionNavigationBlocked: boolean;
  canUndo: boolean;
  canRedo: boolean;
  runBlock: RunBlock;
  runActivity: RunActivity;
  selectedEntity: boolean;
  evidenceAvailable: boolean;
  fieldAvailable: boolean;
  cylinderRunning: boolean;
  trajectoryAvailable: boolean;
  dcMotorRunning: boolean;
  structuralAvailable: boolean;
  structuralRunning: boolean;
  fsiAvailable: boolean;
  fsiRunning: boolean;
  cadAvailability: WorkflowAvailability;
}>;

export type CommandAvailability = Readonly<
  Record<CommandId, Readonly<{ enabled: boolean; reason: MessageKey | null }>>
>;

function commandState(
  enabled: boolean,
  reason: MessageKey,
): Readonly<{ enabled: boolean; reason: MessageKey | null }> {
  return { enabled, reason: enabled ? null : reason };
}

function runReason(block: Exclude<RunBlock, null>): MessageKey {
  switch (block) {
    case "active-run":
      return "command.reason.run-active";
    case "committing":
      return "command.reason.commit-active";
    case "no-document":
      return "command.reason.compile-first";
    case "source-edited":
      return "command.reason.compile-source";
    case "invalid-spatial":
      return "command.reason.spatial-input";
    case "spatial-plan":
      return "command.reason.spatial-plan";
    case "invalid-time":
      return "command.reason.time-input";
    case "time-plan":
      return "command.reason.time-plan";
  }
}

/**
 * Resolve command availability once for navigation, toolbar, and palette.
 * Facts are typed application state; this function never inspects source text,
 * aliases, component trees, or renderer objects.
 */
export function resolveCommandAvailability(facts: CommandFacts): CommandAvailability {
  const inWorkflow = (command: CommandId) => {
    const definition = COMMAND_REGISTRY.find((candidate) => candidate.id === command);
    return (
      definition !== undefined &&
      (definition.workflows as readonly WorkflowId[]).includes(facts.activeWorkflow)
    );
  };
  const scoped = (
    command: CommandId,
    state: Readonly<{ enabled: boolean; reason: MessageKey | null }>,
  ) =>
    inWorkflow(command)
      ? state
      : { enabled: false, reason: "command.reason.workflow-unavailable" as const };
  const run =
    facts.runBlock === null
      ? { enabled: true, reason: null }
      : { enabled: false, reason: runReason(facts.runBlock) };
  const editReason =
    facts.valueEditBlock === "run"
      ? "command.reason.edit-run"
      : facts.valueEditBlock === "source"
        ? "command.reason.edit-source"
        : "command.reason.edit-preview";
  const navigationReason = facts.revisionNavigationBlocked
    ? "command.reason.operation-active"
    : null;
  const cadEnabled =
    facts.cadAvailability.kind === "available" || facts.cadAvailability.kind === "loading";
  const cadReason = facts.cadAvailability.reason ?? "workflow.reason.cad-unavailable";

  return {
    "model.compile": commandState(!facts.compiling, "command.reason.compiling"),
    "edit.commit": scoped(
      "edit.commit",
      commandState(facts.valueEditReady && facts.valueEditBlock === null, editReason),
    ),
    "history.undo": commandState(
      facts.canUndo,
      navigationReason ?? "command.reason.first-revision",
    ),
    "history.redo": commandState(
      facts.canRedo,
      navigationReason ?? "command.reason.no-child-revision",
    ),
    "run.execute": scoped("run.execute", run),
    "run.cancel": scoped(
      "run.cancel",
      facts.runActivity === "reference-running"
        ? { enabled: true, reason: null }
        : {
            enabled: false,
            reason:
              facts.runActivity === "spatial-running"
                ? "command.reason.spatial-cancellation"
                : facts.runActivity === "reference-cancelling"
                  ? "command.reason.cancellation-requested"
                  : "command.reason.no-active-run",
          },
    ),
    "view.reflow": scoped(
      "view.reflow",
      commandState(facts.documentAccepted, "command.reason.compile-first"),
    ),
    "workspace.relations": { enabled: true, reason: null },
    "workspace.field": commandState(
      facts.fieldAvailable,
      "command.reason.field-result-unavailable",
    ),
    "workspace.trajectory": commandState(
      facts.trajectoryAvailable,
      "command.reason.trajectory-result-unavailable",
    ),
    "workspace.structure": commandState(
      facts.structuralAvailable,
      "command.reason.structural-result-unavailable",
    ),
    "workspace.fsi": commandState(facts.fsiAvailable, "command.reason.fsi-result-unavailable"),
    "workspace.geometry": commandState(cadEnabled, cadReason),
    "example.cylinder": commandState(!facts.cylinderRunning, "command.reason.cylinder-running"),
    "example.dc-drive": commandState(!facts.dcMotorRunning, "command.reason.dc-drive-running"),
    "example.structural": commandState(
      !facts.structuralRunning,
      "command.reason.structural-running",
    ),
    "example.fsi": commandState(!facts.fsiRunning, "command.reason.fsi-running"),
    "example.spatial": commandState(!facts.compiling, "command.reason.compiling"),
    "example.cad": commandState(!facts.compiling, "command.reason.compiling"),
    "focus.source": scoped("focus.source", { enabled: true, reason: null }),
    "focus.relation": scoped("focus.relation", { enabled: true, reason: null }),
    "focus.inspector": scoped(
      "focus.inspector",
      commandState(facts.selectedEntity, "command.reason.select-entity"),
    ),
    "focus.evidence": scoped(
      "focus.evidence",
      commandState(facts.evidenceAvailable, "command.reason.complete-run"),
    ),
  };
}
