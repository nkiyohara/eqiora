import { COMMAND_REGISTRY, WORKFLOW_REGISTRY } from "./application-registry";
import type { MessageKey } from "./messages";
import type { DocumentProjection } from "./protocol";

export { COMMAND_REGISTRY, WORKFLOW_REGISTRY } from "./application-registry";
export type WorkflowId = "relations" | "packaged-dc-drive" | "cad-box" | "cad-authored";
export type WorkspaceId = "relations" | "trajectory" | "geometry" | "cad-authoring";
export type CommandId =
  | "model.compile"
  | "edit.commit"
  | "history.undo"
  | "history.redo"
  | "view.reflow"
  | "workspace.relations"
  | "workspace.trajectory"
  | "workspace.geometry"
  | "workspace.cad-authoring"
  | "example.dc-drive"
  | "example.cad"
  | "focus.source"
  | "focus.relation"
  | "focus.inspector"
  | "focus.evidence";
export type CommandGroup = "model" | "view" | "navigate";
export type FocusTarget =
  | "source-editor"
  | "relation-view"
  | "selection-inspector"
  | "evidence-inspector"
  | "trajectory-viewport"
  | "trajectory-sample-table"
  | "cad-viewport"
  | "cad-domain-table"
  | "cad-authored-workspace";
export type ElementFocusTarget = Exclude<FocusTarget, "source-editor" | "relation-view">;
export const DC_MOTOR_EVIDENCE_FOCUS_ID = "dc-drive-evidence-inspector";
export function resolveElementFocusId(target: ElementFocusTarget): string {
  switch (target) {
    case "selection-inspector":
      return "inspector-panel";
    case "evidence-inspector":
      return DC_MOTOR_EVIDENCE_FOCUS_ID;
    case "trajectory-viewport":
      return "trajectory-viewport";
    case "trajectory-sample-table":
      return "trajectory-sample-table";
    case "cad-viewport":
      return "cad-viewport";
    case "cad-domain-table":
      return "cad-domain-table";
    case "cad-authored-workspace":
      return "workspace";
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
export type WorkflowDefinition = Readonly<{
  id: WorkflowId;
  workspace: WorkspaceId;
  label: MessageKey;
  description: MessageKey;
  primaryFocus: FocusTarget;
}>;
export type WorkflowAvailability =
  | Readonly<{ kind: "available"; reason: null }>
  | Readonly<{ kind: "loading" | "unavailable"; reason: MessageKey }>;
export type ResolvedWorkflow = Readonly<{
  definition: WorkflowDefinition;
  availability: WorkflowAvailability;
  commands: readonly CommandDefinition[];
}>;
export type CadApplicationInput = Readonly<{
  status: "idle" | "loading" | "ready" | "unavailable";
  acceptedModelDigest: string | null;
}>;
export type ApplicationInputs = Readonly<{
  acceptedProjection: DocumentProjection | null;
  cad: CadApplicationInput;
  dcMotorStatus: "idle" | "running" | "ready" | "failed";
}>;
export function resolveApplicationWorkflows(
  inputs: ApplicationInputs,
): readonly ResolvedWorkflow[] {
  return WORKFLOW_REGISTRY.map((definition) => {
    let availability: WorkflowAvailability;
    if (definition.id === "cad-authored") availability = { kind: "available", reason: null };
    else if (definition.id === "packaged-dc-drive")
      availability =
        inputs.dcMotorStatus === "running"
          ? { kind: "loading", reason: "workflow.reason.dc-drive-running" }
          : inputs.dcMotorStatus === "ready"
            ? { kind: "available", reason: null }
            : { kind: "unavailable", reason: "workflow.reason.dc-drive-unavailable" };
    else if (inputs.acceptedProjection === null)
      availability = { kind: "unavailable", reason: "workflow.reason.compile-first" };
    else if (definition.id === "cad-box")
      availability =
        inputs.cad.status === "loading" || inputs.cad.status === "idle"
          ? { kind: "loading", reason: "workflow.reason.cad-loading" }
          : inputs.cad.status === "ready" &&
              inputs.cad.acceptedModelDigest === inputs.acceptedProjection.digest
            ? { kind: "available", reason: null }
            : {
                kind: "unavailable",
                reason:
                  inputs.cad.status === "ready"
                    ? "workflow.reason.cad-stale"
                    : "workflow.reason.cad-unavailable",
              };
    else availability = { kind: "available", reason: null };
    return {
      definition,
      availability,
      commands: COMMAND_REGISTRY.filter((command) =>
        (command.workflows as readonly WorkflowId[]).includes(definition.id),
      ),
    };
  });
}
export function resolveApplication(inputs: ApplicationInputs, requestedWorkspace: WorkspaceId) {
  const workflows = resolveApplicationWorkflows(inputs);
  const kind = (id: WorkflowId) =>
    workflows.find((item) => item.definition.id === id)?.availability.kind;
  const workspace =
    requestedWorkspace === "geometry" && !["available", "loading"].includes(kind("cad-box") ?? "")
      ? "relations"
      : requestedWorkspace === "trajectory" &&
          !["available", "loading"].includes(kind("packaged-dc-drive") ?? "")
        ? "relations"
        : requestedWorkspace;
  const activeWorkflow: WorkflowId =
    workspace === "trajectory"
      ? "packaged-dc-drive"
      : workspace === "geometry"
        ? "cad-box"
        : workspace === "cad-authoring"
          ? "cad-authored"
          : "relations";
  return {
    requestedWorkspace,
    workspace,
    activeWorkflow,
    workflows,
    fellBack: workspace !== requestedWorkspace,
  } as const;
}
export type ValueEditBlock = "source" | null;
export type CommandFacts = Readonly<{
  activeWorkflow: WorkflowId;
  compiling: boolean;
  documentAccepted: boolean;
  valueEditReady: boolean;
  valueEditBlock: ValueEditBlock;
  revisionNavigationBlocked: boolean;
  canUndo: boolean;
  canRedo: boolean;
  selectedEntity: boolean;
  evidenceAvailable: boolean;
  trajectoryAvailable: boolean;
  dcMotorRunning: boolean;
  cadAvailability: WorkflowAvailability;
}>;
export type CommandAvailability = Readonly<
  Record<CommandId, Readonly<{ enabled: boolean; reason: MessageKey | null }>>
>;
export function resolveCommandAvailability(facts: CommandFacts): CommandAvailability {
  const state = (enabled: boolean, reason: MessageKey) => ({
    enabled,
    reason: enabled ? null : reason,
  });
  return {
    "model.compile": state(!facts.compiling, "command.reason.compiling"),
    "edit.commit": state(
      facts.valueEditReady && facts.valueEditBlock === null,
      facts.valueEditBlock === "source"
        ? "command.reason.edit-source"
        : "command.reason.edit-preview",
    ),
    "history.undo": state(facts.canUndo, "command.reason.first-revision"),
    "history.redo": state(facts.canRedo, "command.reason.no-child-revision"),
    "view.reflow": state(facts.documentAccepted, "command.reason.compile-first"),
    "workspace.relations": { enabled: true, reason: null },
    "workspace.trajectory": state(
      facts.trajectoryAvailable,
      "command.reason.trajectory-result-unavailable",
    ),
    "workspace.geometry": state(
      ["available", "loading"].includes(facts.cadAvailability.kind),
      facts.cadAvailability.reason ?? "workflow.reason.cad-unavailable",
    ),
    "workspace.cad-authoring": { enabled: true, reason: null },
    "example.dc-drive": state(!facts.dcMotorRunning, "command.reason.dc-drive-running"),
    "example.cad": state(!facts.compiling, "command.reason.compiling"),
    "focus.source": { enabled: true, reason: null },
    "focus.relation": { enabled: true, reason: null },
    "focus.inspector": state(facts.selectedEntity, "command.reason.select-entity"),
    "focus.evidence": state(facts.evidenceAvailable, "command.reason.complete-run"),
  };
}
