/** Presentation-only labels; these keys never enter semantic identity or IPC. */
export const ENGLISH_MESSAGES = {
  "command.group.model": "Model",
  "command.group.navigate": "Navigate",
  "command.group.view": "View",
  "command.example.cad.description":
    "Load the immutable bounded CAD example through the ordinary compile path.",
  "command.example.cad.label": "Open CAD example",
  "command.example.dc-drive.description":
    "Compile and execute the pinned three-package sampled DC-drive example.",
  "command.example.dc-drive.label": "Run DC-drive demo",
  "command.edit.commit.description":
    "Atomically apply the exact transaction preview as a child revision.",
  "command.edit.commit.label": "Commit accepted value edit",
  "command.focus.evidence.description":
    "Move keyboard focus to immutable evidence for the completed DC-drive run.",
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
  "command.reason.compile-first": "Compile a canonical revision first.",
  "command.reason.compiling": "Compilation is in progress.",
  "command.reason.complete-run": "Complete an accepted run first.",
  "command.reason.dc-drive-running": "The packaged DC-drive demonstration is already running.",
  "command.reason.edit-preview": "Enter a distinct valid value and wait for transaction preview.",
  "command.reason.edit-source":
    "Compile or discard source changes before creating a child revision.",
  "command.reason.first-revision": "This is the first retained revision.",
  "command.reason.no-child-revision": "There is no retained child revision.",
  "command.reason.select-entity": "Select a canonical entity first.",
  "command.reason.trajectory-result-unavailable":
    "Run the packaged DC-drive example to view its production trajectory.",
  "command.view.reflow.description":
    "Reset workspace-only positions without changing model identity.",
  "command.view.reflow.label": "Reflow relation view",
  "command.workspace.geometry.description":
    "Show the geometry workspace for the accepted bounded CAD plan.",
  "command.workspace.geometry.label": "Show geometry workspace",
  "command.workspace.cad-authoring.description":
    "Open the bounded authored-CAD history inspector and native replay controls.",
  "command.workspace.cad-authoring.label": "Show authored CAD workspace",
  "command.workspace.relations.description":
    "Show source, relation, inspector, and diagnostics projections.",
  "command.workspace.relations.label": "Show relations workspace",
  "command.workspace.trajectory.description":
    "Open the bounded production trajectory for the packaged DC-drive example.",
  "command.workspace.trajectory.label": "Show trajectory workspace",
  "workflow.cad.description":
    "Inspect the exact bounded CAD plan through semantic geometry and Domain selection.",
  "workflow.cad.label": "Geometry",
  "workflow.cad-authored.description":
    "Author and inspect the two accepted CAD histories through native owner replay.",
  "workflow.cad-authored.label": "Authored CAD",
  "workflow.dc-drive.description":
    "Inspect one pinned package closure, sampled trajectory, and exact Model/Run binding.",
  "workflow.dc-drive.label": "DC drive",
  "workflow.reason.cad-loading": "The native runtime is resolving the exact CAD plan.",
  "workflow.reason.cad-stale": "The accepted CAD plan belongs to another Model revision.",
  "workflow.reason.cad-unavailable": "This canonical revision has no accepted bounded CAD plan.",
  "workflow.reason.compile-first": "Compile a canonical revision first.",
  "workflow.reason.dc-drive-running":
    "The native runtime is compiling, executing, and binding the packaged DC drive.",
  "workflow.reason.dc-drive-unavailable":
    "Run the packaged DC-drive demonstration to open this workflow.",
  "workflow.relations.description":
    "Inspect source, canonical entities, relations, and typed properties.",
  "workflow.relations.label": "Relations",
} as const;
export type MessageKey = keyof typeof ENGLISH_MESSAGES;
export type MessageCatalog = Readonly<Record<MessageKey, string>>;
export function formatMessage<Key extends MessageKey>(key: Key): (typeof ENGLISH_MESSAGES)[Key];
export function formatMessage(key: MessageKey, catalog: MessageCatalog): string;
export function formatMessage(key: MessageKey, catalog: MessageCatalog = ENGLISH_MESSAGES): string {
  return catalog[key];
}
