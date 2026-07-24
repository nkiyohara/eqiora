import {
  CAD_V1_SEMANTIC_ENTITY_COUNT,
  CAD_V1_TRIANGLE_COUNT,
  CAD_V1_VERTEX_COUNT,
} from "./cad-protocol";
import type { MessageKey } from "./messages";
import {
  type DocumentProjection,
  MAX_PROJECTION_EDGE_COUNT,
  MAX_PROJECTION_NODE_COUNT,
} from "./protocol";
import { SCALAR_FIELD_VALUES_PER_CHUNK } from "./scalar-field-protocol";
import { MAX_SPATIAL_ENTITY_COUNT } from "./spatial-protocol";

export type WorkflowId = "relations" | "scalar-elliptic" | "cad-box";
export type WorkspaceId = "relations" | "field" | "geometry";

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
  | "workspace.geometry"
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
  | "cad-viewport"
  | "cad-domain-table";

export interface CommandDefinition {
  readonly id: CommandId;
  readonly group: CommandGroup;
  readonly label: MessageKey;
  readonly description: MessageKey;
  readonly shortcut: string | null;
  readonly focusTarget: FocusTarget | null;
  readonly workflows: readonly WorkflowId[];
}

const RELATION_WORKFLOWS = ["relations", "scalar-elliptic"] as const;
const ALL_WORKFLOWS = ["relations", "scalar-elliptic", "cad-box"] as const;

/**
 * Closed command metadata for the currently implemented Studio operations.
 *
 * Execution remains an exhaustive application adapter. Definitions contain no
 * callback, model mutation, capability inference, or dynamically discovered
 * payload.
 */
export const COMMAND_REGISTRY = [
  {
    id: "model.compile",
    group: "model",
    label: "command.model.compile.label",
    description: "command.model.compile.description",
    shortcut: "Ctrl/⌘ Enter",
    focusTarget: null,
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "edit.commit",
    group: "model",
    label: "command.edit.commit.label",
    description: "command.edit.commit.description",
    shortcut: null,
    focusTarget: null,
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "history.undo",
    group: "model",
    label: "command.history.undo.label",
    description: "command.history.undo.description",
    shortcut: "Ctrl/⌘ Z",
    focusTarget: null,
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "history.redo",
    group: "model",
    label: "command.history.redo.label",
    description: "command.history.redo.description",
    shortcut: "Ctrl/⌘ ⇧ Z",
    focusTarget: null,
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "run.execute",
    group: "execution",
    label: "command.run.execute.label",
    description: "command.run.execute.description",
    shortcut: null,
    focusTarget: null,
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "run.cancel",
    group: "execution",
    label: "command.run.cancel.label",
    description: "command.run.cancel.description",
    shortcut: null,
    focusTarget: null,
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "view.reflow",
    group: "view",
    label: "command.view.reflow.label",
    description: "command.view.reflow.description",
    shortcut: null,
    focusTarget: null,
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "workspace.relations",
    group: "view",
    label: "command.workspace.relations.label",
    description: "command.workspace.relations.description",
    shortcut: null,
    focusTarget: "relation-view",
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "workspace.geometry",
    group: "view",
    label: "command.workspace.geometry.label",
    description: "command.workspace.geometry.description",
    shortcut: null,
    focusTarget: "cad-viewport",
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "workspace.field",
    group: "view",
    label: "command.workspace.field.label",
    description: "command.workspace.field.description",
    shortcut: null,
    focusTarget: "field-viewport",
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "example.spatial",
    group: "model",
    label: "command.example.spatial.label",
    description: "command.example.spatial.description",
    shortcut: null,
    focusTarget: null,
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "example.cad",
    group: "model",
    label: "command.example.cad.label",
    description: "command.example.cad.description",
    shortcut: null,
    focusTarget: null,
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "focus.source",
    group: "navigate",
    label: "command.focus.source.label",
    description: "command.focus.source.description",
    shortcut: null,
    focusTarget: "source-editor",
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "focus.relation",
    group: "navigate",
    label: "command.focus.relation.label",
    description: "command.focus.relation.description",
    shortcut: null,
    focusTarget: "relation-view",
    workflows: RELATION_WORKFLOWS,
  },
  {
    id: "focus.inspector",
    group: "navigate",
    label: "command.focus.inspector.label",
    description: "command.focus.inspector.description",
    shortcut: null,
    focusTarget: "selection-inspector",
    workflows: ALL_WORKFLOWS,
  },
  {
    id: "focus.evidence",
    group: "navigate",
    label: "command.focus.evidence.label",
    description: "command.focus.evidence.description",
    shortcut: null,
    focusTarget: "evidence-inspector",
    workflows: RELATION_WORKFLOWS,
  },
] as const satisfies readonly CommandDefinition[];

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
      kind: "bounded-cad-triangle-view";
      maximumVertices: typeof CAD_V1_VERTEX_COUNT;
      maximumTriangles: typeof CAD_V1_TRIANGLE_COUNT;
      maximumEntities: typeof CAD_V1_SEMANTIC_ENTITY_COUNT;
      semanticAlternative: Extract<SemanticAlternative, { kind: "domain-table" }>;
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
  | CadWorkflowDefinition;

/**
 * The registry is deliberately exhaustive and compiled in. It is not package
 * discovery, a plugin ABI, or an authority for canonical model meaning.
 */
export const WORKFLOW_REGISTRY = [
  {
    id: "relations",
    workspace: "relations",
    label: "workflow.relations.label",
    description: "workflow.relations.description",
    primaryFocus: "relation-view",
    projection: {
      kind: "semantic-relation-graph",
      maximumNodes: MAX_PROJECTION_NODE_COUNT,
      maximumEdges: MAX_PROJECTION_EDGE_COUNT,
      semanticAlternative: {
        kind: "semantic-outline",
        focusTarget: "source-editor",
      },
    },
  },
  {
    id: "scalar-elliptic",
    workspace: "relations",
    label: "workflow.spatial.label",
    description: "workflow.spatial.description",
    primaryFocus: "relation-view",
    projection: {
      kind: "bounded-scalar-field-view",
      maximumFieldValues: MAX_SPATIAL_ENTITY_COUNT,
      transfer: "explicit-owned-host-copy",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      semanticAlternative: {
        kind: "field-value-table",
        focusTarget: "field-value-table",
      },
    },
  },
  {
    id: "cad-box",
    workspace: "geometry",
    label: "workflow.cad.label",
    description: "workflow.cad.description",
    primaryFocus: "cad-viewport",
    projection: {
      kind: "bounded-cad-triangle-view",
      maximumVertices: CAD_V1_VERTEX_COUNT,
      maximumTriangles: CAD_V1_TRIANGLE_COUNT,
      maximumEntities: CAD_V1_SEMANTIC_ENTITY_COUNT,
      semanticAlternative: {
        kind: "domain-table",
        focusTarget: "cad-domain-table",
      },
    },
  },
] as const satisfies readonly WorkflowDefinition[];

export type AcceptedApplicationProjection = Pick<DocumentProjection, "digest" | "workflows">;

export type CadApplicationStatus = "idle" | "loading" | "ready" | "unavailable";

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
  fieldAvailable: boolean;
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
  const workspace =
    (requestedWorkspace === "geometry" && !geometryAvailable) ||
    (requestedWorkspace === "field" && !inputs.fieldAvailable)
      ? "relations"
      : requestedWorkspace;
  const spatial = workflows.find((workflow) => workflow.definition.id === "scalar-elliptic");
  const activeWorkflow: WorkflowId =
    workspace === "geometry"
      ? "cad-box"
      : workspace === "field"
        ? "scalar-elliptic"
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
    "workspace.geometry": commandState(cadEnabled, cadReason),
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
