/**
 * Presentation-only messages owned by the Studio application shell.
 *
 * Message keys are neither canonical identity nor a wire format. Stable
 * diagnostics remain authoritative; this catalog supplies local labels and
 * recovery language around those typed application states.
 */
export const ENGLISH_MESSAGES = {
  "command.group.execution": "Execution",
  "command.group.model": "Model",
  "command.group.navigate": "Navigate",
  "command.group.view": "View",
  "command.example.cad.description":
    "Load the immutable bounded CAD example through the ordinary compile path.",
  "command.example.cad.label": "Open CAD example",
  "command.example.cylinder.description":
    "Run the immutable exact-cylinder Stokes example and open its accepted pressure field.",
  "command.example.cylinder.label": "Run cylinder demo",
  "command.example.dc-drive.description":
    "Compile and execute the pinned three-package sampled DC-drive example.",
  "command.example.dc-drive.label": "Run DC-drive demo",
  "command.example.structural.description":
    "Execute the verified mixed-boundary 2D elasticity example.",
  "command.example.structural.label": "Run structural demo",
  "command.example.spatial.description":
    "Load the manufactured scalar elliptic example through the ordinary compile path.",
  "command.example.spatial.label": "Open spatial example",
  "command.edit.commit.description":
    "Atomically apply the exact transaction preview as a child revision.",
  "command.edit.commit.label": "Commit accepted value edit",
  "command.focus.evidence.description":
    "Move keyboard focus to immutable evidence for the completed run.",
  "command.focus.evidence.label": "Focus run evidence",
  "command.focus.inspector.description": "Move keyboard focus to the selected entity projection.",
  "command.focus.inspector.label": "Focus selection inspector",
  "command.focus.relation.description": "Move keyboard focus to the canonical relation projection.",
  "command.focus.relation.label": "Focus relation view",
  "command.focus.source.description": "Move keyboard focus to canonical source input.",
  "command.focus.source.label": "Focus model source",
  "command.history.redo.description":
    "Navigate forward along the retained canonical revision lineage.",
  "command.history.redo.label": "Next revision",
  "command.history.undo.description":
    "Navigate to the parent canonical revision without creating an inverse edit.",
  "command.history.undo.label": "Previous revision",
  "command.model.compile.description": "Validate source and create one canonical revision.",
  "command.model.compile.label": "Compile model",
  "command.reason.cancellation-requested": "Cancellation has already been requested.",
  "command.reason.commit-active": "Wait for the revision commit to finish.",
  "command.reason.compile-first": "Compile a canonical revision first.",
  "command.reason.compile-source": "Compile source changes before running.",
  "command.reason.compiling": "Compilation is in progress.",
  "command.reason.complete-run": "Complete an accepted run first.",
  "command.reason.cylinder-running": "The exact-cylinder demonstration is already running.",
  "command.reason.dc-drive-running": "The packaged DC-drive demonstration is already running.",
  "command.reason.structural-running": "The structural demonstration is already running.",
  "command.reason.structural-result-unavailable":
    "Run the structural example to view its accepted displacement.",
  "command.reason.edit-preview": "Enter a distinct valid value and wait for transaction preview.",
  "command.reason.edit-run": "Wait for the current run to finish before changing revisions.",
  "command.reason.edit-source":
    "Compile or discard source changes before creating a child revision.",
  "command.reason.field-result-unavailable":
    "Run an admitted 2D scalar Realization to view its field.",
  "command.reason.first-revision": "This is the first retained revision.",
  "command.reason.no-active-run": "There is no active run to cancel.",
  "command.reason.no-child-revision": "There is no retained child revision.",
  "command.reason.operation-active": "Wait for the current operation to finish.",
  "command.reason.run-active": "A run is already in progress.",
  "command.reason.select-entity": "Select a canonical entity first.",
  "command.reason.spatial-cancellation":
    "This bounded spatial slice has no accepted cancellation boundary.",
  "command.reason.spatial-input": "Enter a bounded spatial Realization.",
  "command.reason.spatial-plan": "Wait for the native runtime to accept this Realization.",
  "command.reason.time-input": "Enter valid model-time controls.",
  "command.reason.time-plan": "Wait for the native runtime to accept this plan.",
  "command.reason.trajectory-result-unavailable":
    "Run the packaged DC-drive example to view its production trajectory.",
  "command.reason.workflow-unavailable": "This operation is not available in the active workflow.",
  "command.run.cancel.description":
    "Request cancellation at the next safe, accepted semantic step.",
  "command.run.cancel.label": "Cancel active run",
  "command.run.execute.description":
    "Execute the exact plan admitted by the native capability preview.",
  "command.run.execute.label": "Run accepted plan",
  "command.view.reflow.description":
    "Reset workspace-only positions without changing model identity.",
  "command.view.reflow.label": "Reflow relation view",
  "command.workspace.geometry.description":
    "Show the geometry workspace for the accepted bounded CAD plan.",
  "command.workspace.geometry.label": "Show geometry workspace",
  "command.workspace.cad-authoring.description":
    "Open the bounded authored-CAD history inspector and native replay controls.",
  "command.workspace.cad-authoring.label": "Show authored CAD workspace",
  "command.workspace.field.description":
    "Open the bounded field view for one accepted 2D scalar result.",
  "command.workspace.field.label": "Show field workspace",
  "command.workspace.relations.description":
    "Show source, relation, inspector, diagnostics, and evidence projections.",
  "command.workspace.relations.label": "Show relations workspace",
  "command.workspace.trajectory.description":
    "Open the bounded production trajectory for the packaged DC-drive example.",
  "command.workspace.trajectory.label": "Show trajectory workspace",
  "command.workspace.structure.description":
    "Open the bounded displacement-grid view for the accepted structural solve.",
  "command.workspace.structure.label": "Show structural workspace",
  "workflow.cad.description":
    "Inspect the exact bounded CAD plan through semantic geometry and Domain selection.",
  "workflow.cad.label": "Geometry",
  "workflow.cad-authored.description":
    "Author and inspect the two accepted CAD histories through native owner replay.",
  "workflow.cad-authored.label": "Authored CAD",
  "workflow.cylinder.description":
    "Inspect one exact-circle, error-controlled affine Stokes solve and its accepted pressure P1 field.",
  "workflow.cylinder.label": "Cylinder Stokes",
  "workflow.dc-drive.description":
    "Inspect one pinned package closure, sampled trajectory, and exact Model/Run binding.",
  "workflow.dc-drive.label": "DC drive",
  "workflow.reason.cad-loading": "The native runtime is resolving the exact CAD plan.",
  "workflow.reason.cad-stale": "The accepted CAD plan belongs to another Model revision.",
  "workflow.reason.cad-unavailable": "This canonical revision has no accepted bounded CAD plan.",
  "workflow.reason.compile-first": "Compile a canonical revision first.",
  "workflow.reason.cylinder-running":
    "The native runtime is solving and binding the exact-cylinder result.",
  "workflow.reason.cylinder-unavailable":
    "Run the immutable exact-cylinder demonstration to open this workflow.",
  "workflow.reason.dc-drive-running":
    "The native runtime is compiling, executing, and binding the packaged DC drive.",
  "workflow.reason.dc-drive-unavailable":
    "Run the packaged DC-drive demonstration to open this workflow.",
  "workflow.reason.structural-running":
    "The native runtime is resolving and solving the structural example.",
  "workflow.reason.structural-unavailable":
    "Run the mixed-boundary elasticity demonstration to open this workflow.",
  "workflow.reason.spatial-unavailable":
    "This canonical revision does not lower to the bounded scalar elliptic workflow.",
  "workflow.relations.description":
    "Inspect source, canonical entities, relations, typed properties, and run evidence.",
  "workflow.relations.label": "Relations",
  "workflow.spatial.description":
    "Resolve, execute, and verify one bounded scalar elliptic Realization.",
  "workflow.spatial.label": "Scalar elliptic",
  "workflow.structural.description":
    "Inspect one verified Q1 displacement field with solver-owned balance and lineage evidence.",
  "workflow.structural.label": "Structure",
} as const;

export type MessageKey = keyof typeof ENGLISH_MESSAGES;
export type MessageCatalog = Readonly<Record<MessageKey, string>>;

export function formatMessage<Key extends MessageKey>(key: Key): (typeof ENGLISH_MESSAGES)[Key];
export function formatMessage(key: MessageKey, catalog: MessageCatalog): string;
export function formatMessage(key: MessageKey, catalog: MessageCatalog = ENGLISH_MESSAGES): string {
  return catalog[key];
}
