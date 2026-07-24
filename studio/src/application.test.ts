import { describe, expect, it } from "vitest";
import {
  type AcceptedApplicationProjection,
  COMMAND_REGISTRY,
  resolveApplication,
  resolveApplicationWorkflows,
  resolveCommandAvailability,
  WORKFLOW_REGISTRY,
} from "./application";
import {
  CAD_V1_SEMANTIC_ENTITY_COUNT,
  CAD_V1_TRIANGLE_COUNT,
  CAD_V1_VERTEX_COUNT,
} from "./cad-protocol";
import { ENGLISH_MESSAGES, formatMessage } from "./messages";
import { MAX_PROJECTION_EDGE_COUNT, MAX_PROJECTION_NODE_COUNT } from "./protocol";
import { SCALAR_FIELD_VALUES_PER_CHUNK } from "./scalar-field-protocol";
import { MAX_SPATIAL_ENTITY_COUNT } from "./spatial-protocol";

const DIGEST = "a".repeat(64);

function acceptedProjection(scalarElliptic: boolean): AcceptedApplicationProjection {
  return {
    digest: DIGEST,
    workflows: {
      scalarElliptic: scalarElliptic
        ? {
            spatialDimension: 2,
            scalarType: "f64",
            vectorLayout: "replicated",
            maximumHostWorkers: 4,
            workerBudgetSource: "studio-session-budget",
          }
        : null,
    },
  };
}

function resolved(
  scalarElliptic: boolean,
  cad: Parameters<typeof resolveApplicationWorkflows>[0]["cad"],
) {
  return resolveApplicationWorkflows({
    acceptedProjection: acceptedProjection(scalarElliptic),
    cad,
    fieldAvailable: false,
  });
}

describe("typed Studio application registry", () => {
  it("is a closed, unique inventory of the current workflows and commands", () => {
    expect(WORKFLOW_REGISTRY.map((workflow) => workflow.id)).toEqual([
      "relations",
      "scalar-elliptic",
      "cad-box",
    ]);
    expect(new Set(WORKFLOW_REGISTRY.map((workflow) => workflow.id)).size).toBe(
      WORKFLOW_REGISTRY.length,
    );
    expect(COMMAND_REGISTRY.map((command) => command.id)).toEqual([
      "model.compile",
      "edit.commit",
      "history.undo",
      "history.redo",
      "run.execute",
      "run.cancel",
      "view.reflow",
      "workspace.relations",
      "workspace.geometry",
      "workspace.field",
      "example.spatial",
      "example.cad",
      "focus.source",
      "focus.relation",
      "focus.inspector",
      "focus.evidence",
    ]);
    expect(new Set(COMMAND_REGISTRY.map((command) => command.id)).size).toBe(
      COMMAND_REGISTRY.length,
    );
  });

  it("declares a bounded projection and semantic alternative for every workflow", () => {
    const relations = WORKFLOW_REGISTRY[0];
    const spatial = WORKFLOW_REGISTRY[1];
    const cad = WORKFLOW_REGISTRY[2];

    expect(relations.projection).toEqual({
      kind: "semantic-relation-graph",
      maximumNodes: MAX_PROJECTION_NODE_COUNT,
      maximumEdges: MAX_PROJECTION_EDGE_COUNT,
      semanticAlternative: {
        kind: "semantic-outline",
        focusTarget: "source-editor",
      },
    });
    expect(spatial.projection).toEqual({
      kind: "bounded-scalar-field-view",
      maximumFieldValues: MAX_SPATIAL_ENTITY_COUNT,
      transfer: "explicit-owned-host-copy",
      valuesPerChunk: SCALAR_FIELD_VALUES_PER_CHUNK,
      semanticAlternative: {
        kind: "field-value-table",
        focusTarget: "field-value-table",
      },
    });
    expect(cad.projection).toEqual({
      kind: "bounded-cad-triangle-view",
      maximumVertices: CAD_V1_VERTEX_COUNT,
      maximumTriangles: CAD_V1_TRIANGLE_COUNT,
      maximumEntities: CAD_V1_SEMANTIC_ENTITY_COUNT,
      semanticAlternative: {
        kind: "domain-table",
        focusTarget: "cad-domain-table",
      },
    });
  });

  it("derives applicability only from accepted projection and exact CAD session identity", () => {
    const noDocument = resolveApplicationWorkflows({
      acceptedProjection: null,
      cad: { status: "ready", acceptedModelDigest: DIGEST },
      fieldAvailable: false,
    });
    expect(noDocument.map((workflow) => workflow.availability)).toEqual([
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
    ]);

    const unavailable = resolved(false, {
      status: "unavailable",
      acceptedModelDigest: null,
    });
    expect(unavailable.map((workflow) => workflow.availability)).toEqual([
      { kind: "available", reason: null },
      { kind: "unavailable", reason: "workflow.reason.spatial-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.cad-unavailable" },
    ]);

    const accepted = resolved(true, {
      status: "ready",
      acceptedModelDigest: DIGEST,
    });
    expect(accepted.every((workflow) => workflow.availability.kind === "available")).toBe(true);

    const staleCad = resolved(true, {
      status: "ready",
      acceptedModelDigest: "b".repeat(64),
    });
    expect(staleCad[2]?.availability).toEqual({
      kind: "unavailable",
      reason: "workflow.reason.cad-stale",
    });
  });

  it("keeps pending CAD resolution distinct from unsupported applicability", () => {
    for (const status of ["idle", "loading"] as const) {
      const cad = resolved(false, { status, acceptedModelDigest: null })[2];
      expect(cad?.availability).toEqual({
        kind: "loading",
        reason: "workflow.reason.cad-loading",
      });
    }
  });

  it("falls back from an inapplicable Geometry workspace without touching model state", () => {
    const unavailable = resolveApplication(
      {
        acceptedProjection: acceptedProjection(false),
        cad: { status: "unavailable", acceptedModelDigest: null },
        fieldAvailable: false,
      },
      "geometry",
    );
    expect(unavailable.workspace).toBe("relations");
    expect(unavailable.activeWorkflow).toBe("relations");
    expect(unavailable.fellBack).toBe(true);
    expect(unavailable.requestedWorkspace).toBe("geometry");

    const loading = resolveApplication(
      {
        acceptedProjection: acceptedProjection(false),
        cad: { status: "loading", acceptedModelDigest: null },
        fieldAvailable: false,
      },
      "geometry",
    );
    expect(loading.workspace).toBe("geometry");
    expect(loading.activeWorkflow).toBe("cad-box");
    expect(loading.fellBack).toBe(false);

    const field = resolveApplication(
      {
        acceptedProjection: acceptedProjection(true),
        cad: { status: "unavailable", acceptedModelDigest: null },
        fieldAvailable: true,
      },
      "field",
    );
    expect(field.workspace).toBe("field");
    expect(field.activeWorkflow).toBe("scalar-elliptic");

    const missingField = resolveApplication(
      {
        acceptedProjection: acceptedProjection(true),
        cad: { status: "unavailable", acceptedModelDigest: null },
        fieldAvailable: false,
      },
      "field",
    );
    expect(missingField.workspace).toBe("relations");
    expect(missingField.fellBack).toBe(true);
  });

  it("resolves toolbar, navigation, and palette availability from the same facts", () => {
    const cadAvailability = resolved(true, {
      status: "unavailable",
      acceptedModelDigest: null,
    })[2]?.availability;
    expect(cadAvailability).toBeDefined();
    if (cadAvailability === undefined) return;

    const availability = resolveCommandAvailability({
      activeWorkflow: "scalar-elliptic",
      compiling: false,
      documentAccepted: true,
      valueEditReady: false,
      valueEditBlock: null,
      revisionNavigationBlocked: false,
      canUndo: false,
      canRedo: false,
      runBlock: "spatial-plan",
      runActivity: "spatial-running",
      selectedEntity: false,
      evidenceAvailable: false,
      fieldAvailable: false,
      cadAvailability,
    });

    expect(Object.keys(availability)).toHaveLength(COMMAND_REGISTRY.length);
    expect(availability["workspace.geometry"]).toEqual({
      enabled: false,
      reason: "workflow.reason.cad-unavailable",
    });
    expect(availability["workspace.field"]).toEqual({
      enabled: false,
      reason: "command.reason.field-result-unavailable",
    });
    expect(availability["run.execute"]).toEqual({
      enabled: false,
      reason: "command.reason.spatial-plan",
    });
    expect(availability["run.cancel"]).toEqual({
      enabled: false,
      reason: "command.reason.spatial-cancellation",
    });
    expect(availability["focus.inspector"]).toEqual({
      enabled: false,
      reason: "command.reason.select-entity",
    });
  });

  it("scopes commands and focus targets to compatible current workflows", () => {
    const workflows = resolved(true, {
      status: "ready",
      acceptedModelDigest: DIGEST,
    });
    const relations = workflows[0]?.commands.map((command) => command.id);
    const spatial = workflows[1]?.commands.map((command) => command.id);
    const cad = workflows[2]?.commands.map((command) => command.id);

    expect(relations).toContain("run.cancel");
    expect(spatial).toContain("run.cancel");
    expect(cad).toEqual([
      "model.compile",
      "history.undo",
      "history.redo",
      "workspace.relations",
      "workspace.geometry",
      "workspace.field",
      "example.spatial",
      "example.cad",
      "focus.inspector",
    ]);

    expect(
      COMMAND_REGISTRY.filter((command) => command.id.startsWith("focus.")).every(
        (command) => command.focusTarget !== null,
      ),
    ).toBe(true);
    expect(
      COMMAND_REGISTRY.find((command) => command.id === "workspace.relations")?.focusTarget,
    ).toBe("relation-view");
    expect(
      COMMAND_REGISTRY.find((command) => command.id === "workspace.geometry")?.focusTarget,
    ).toBe("cad-viewport");
  });

  it("resolves every registry-owned message through the fallback catalog", () => {
    for (const workflow of WORKFLOW_REGISTRY) {
      expect(formatMessage(workflow.label)).toBe(ENGLISH_MESSAGES[workflow.label]);
      expect(formatMessage(workflow.description)).toBe(ENGLISH_MESSAGES[workflow.description]);
    }
    for (const command of COMMAND_REGISTRY) {
      expect(formatMessage(command.label)).toBe(ENGLISH_MESSAGES[command.label]);
      expect(formatMessage(command.description)).toBe(ENGLISH_MESSAGES[command.description]);
    }
    for (const workflow of resolveApplicationWorkflows({
      acceptedProjection: null,
      cad: { status: "idle", acceptedModelDigest: null },
      fieldAvailable: false,
    })) {
      expect(workflow.availability.reason).not.toBeNull();
      if (workflow.availability.reason !== null) {
        expect(formatMessage(workflow.availability.reason)).not.toHaveLength(0);
      }
    }
  });
});
