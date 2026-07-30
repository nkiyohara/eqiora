import { describe, expect, it } from "vitest";
import {
  type AcceptedApplicationProjection,
  type ApplicationInputs,
  COMMAND_REGISTRY,
  type CommandFacts,
  CYLINDER_EVIDENCE_FOCUS_ID,
  DC_MOTOR_EVIDENCE_FOCUS_ID,
  resolveApplication,
  resolveApplicationWorkflows,
  resolveCommandAvailability,
  resolveElementFocusId,
  STRUCTURAL_EVIDENCE_FOCUS_ID,
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
import {
  UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
  UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT,
} from "./unstructured-field-protocol";

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

function applicationInputs(overrides: Partial<ApplicationInputs> = {}): ApplicationInputs {
  return {
    acceptedProjection: acceptedProjection(true),
    cad: { status: "unavailable", acceptedModelDigest: null },
    cylinderStatus: "idle",
    dcMotorStatus: "idle",
    structuralStatus: "idle",
    fieldWorkflow: null,
    ...overrides,
  };
}

function resolved(
  scalarElliptic: boolean,
  cad: ApplicationInputs["cad"],
  cylinderStatus: ApplicationInputs["cylinderStatus"] = "idle",
  dcMotorStatus: ApplicationInputs["dcMotorStatus"] = "idle",
  structuralStatus: ApplicationInputs["structuralStatus"] = "idle",
) {
  return resolveApplicationWorkflows(
    applicationInputs({
      acceptedProjection: acceptedProjection(scalarElliptic),
      cad,
      cylinderStatus,
      dcMotorStatus,
      structuralStatus,
    }),
  );
}

function commandFacts(overrides: Partial<CommandFacts> = {}): CommandFacts {
  return {
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
    cylinderRunning: false,
    trajectoryAvailable: false,
    dcMotorRunning: false,
    structuralAvailable: false,
    structuralRunning: false,
    cadAvailability: {
      kind: "unavailable",
      reason: "workflow.reason.cad-unavailable",
    },
    ...overrides,
  };
}

describe("typed Studio application registry", () => {
  it("is a closed, unique inventory of the current workflows and commands", () => {
    expect(WORKFLOW_REGISTRY.map((workflow) => workflow.id)).toEqual([
      "relations",
      "scalar-elliptic",
      "cylinder-stokes",
      "packaged-dc-drive",
      "structural-elasticity",
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
      "workspace.trajectory",
      "workspace.structure",
      "example.cylinder",
      "example.dc-drive",
      "example.structural",
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
    const [relations, spatial, cylinder, dcDrive, structural, cad] = WORKFLOW_REGISTRY;

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
    expect(cylinder.projection).toEqual({
      kind: "bounded-unstructured-p1-field-view",
      maximumVertices: MAX_SPATIAL_ENTITY_COUNT,
      maximumTriangles: UNSTRUCTURED_FIELD_MAX_TRIANGLE_COUNT,
      transfer: "explicit-owned-host-copy",
      itemsPerChunk: UNSTRUCTURED_FIELD_ITEMS_PER_CHUNK,
      semanticAlternative: {
        kind: "field-value-table",
        focusTarget: "field-value-table",
      },
    });
    expect(dcDrive.projection).toEqual({
      kind: "bounded-sampled-trajectory-view",
      maximumSamples: 101,
      maximumCommits: 11,
      semanticAlternative: {
        kind: "trajectory-sample-table",
        focusTarget: "trajectory-sample-table",
      },
    });
    expect(structural.projection).toEqual({
      kind: "bounded-cartesian-displacement-grid-view",
      maximumVertices: 289,
      maximumCells: 256,
      components: 2,
      semanticAlternative: {
        kind: "structural-vertex-table",
        focusTarget: "structural-vertex-table",
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

  it("derives applicability from accepted projection and exact workflow evidence", () => {
    const noDocument = resolveApplicationWorkflows(
      applicationInputs({
        acceptedProjection: null,
        cad: { status: "ready", acceptedModelDigest: DIGEST },
      }),
    );
    expect(noDocument.map((workflow) => workflow.availability)).toEqual([
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
      { kind: "unavailable", reason: "workflow.reason.cylinder-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.dc-drive-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.structural-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.compile-first" },
    ]);

    const unavailable = resolved(false, {
      status: "unavailable",
      acceptedModelDigest: null,
    });
    expect(unavailable.map((workflow) => workflow.availability)).toEqual([
      { kind: "available", reason: null },
      { kind: "unavailable", reason: "workflow.reason.spatial-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.cylinder-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.dc-drive-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.structural-unavailable" },
      { kind: "unavailable", reason: "workflow.reason.cad-unavailable" },
    ]);

    const accepted = resolved(
      true,
      { status: "ready", acceptedModelDigest: DIGEST },
      "ready",
      "ready",
      "ready",
    );
    expect(accepted.every((workflow) => workflow.availability.kind === "available")).toBe(true);

    const staleCad = resolved(true, {
      status: "ready",
      acceptedModelDigest: "b".repeat(64),
    });
    expect(staleCad[5]?.availability).toEqual({
      kind: "unavailable",
      reason: "workflow.reason.cad-stale",
    });
  });

  it("keeps pending workflow resolution distinct from unsupported applicability", () => {
    for (const status of ["idle", "loading"] as const) {
      const cad = resolved(false, { status, acceptedModelDigest: null })[5];
      expect(cad?.availability).toEqual({
        kind: "loading",
        reason: "workflow.reason.cad-loading",
      });
    }
    expect(
      resolved(false, { status: "unavailable", acceptedModelDigest: null }, "running")[2]
        ?.availability,
    ).toEqual({
      kind: "loading",
      reason: "workflow.reason.cylinder-running",
    });
    expect(
      resolved(false, { status: "unavailable", acceptedModelDigest: null }, "idle", "running")[3]
        ?.availability,
    ).toEqual({
      kind: "loading",
      reason: "workflow.reason.dc-drive-running",
    });
    expect(
      resolved(
        false,
        { status: "unavailable", acceptedModelDigest: null },
        "idle",
        "idle",
        "running",
      )[4]?.availability,
    ).toEqual({
      kind: "loading",
      reason: "workflow.reason.structural-running",
    });
  });

  it("falls back from inapplicable workspaces without touching canonical model state", () => {
    const unavailable = resolveApplication(
      applicationInputs({
        acceptedProjection: acceptedProjection(false),
        fieldWorkflow: null,
      }),
      "geometry",
    );
    expect(unavailable).toMatchObject({
      workspace: "relations",
      activeWorkflow: "relations",
      fellBack: true,
      requestedWorkspace: "geometry",
    });

    const loading = resolveApplication(
      applicationInputs({
        acceptedProjection: acceptedProjection(false),
        cad: { status: "loading", acceptedModelDigest: null },
      }),
      "geometry",
    );
    expect(loading).toMatchObject({
      workspace: "geometry",
      activeWorkflow: "cad-box",
      fellBack: false,
    });

    const scalarField = resolveApplication(
      applicationInputs({ fieldWorkflow: "scalar-elliptic" }),
      "field",
    );
    expect(scalarField).toMatchObject({
      workspace: "field",
      activeWorkflow: "scalar-elliptic",
    });

    const cylinder = resolveApplication(
      applicationInputs({
        acceptedProjection: null,
        cylinderStatus: "running",
        fieldWorkflow: "cylinder-stokes",
      }),
      "field",
    );
    expect(cylinder).toMatchObject({
      workspace: "field",
      activeWorkflow: "cylinder-stokes",
      fellBack: false,
    });

    const failedCylinder = resolveApplication(
      applicationInputs({
        cylinderStatus: "failed",
        fieldWorkflow: "cylinder-stokes",
      }),
      "field",
    );
    expect(failedCylinder).toMatchObject({
      workspace: "relations",
      fellBack: true,
    });

    const runningDcDrive = resolveApplication(
      applicationInputs({
        acceptedProjection: null,
        dcMotorStatus: "running",
      }),
      "trajectory",
    );
    expect(runningDcDrive).toMatchObject({
      workspace: "trajectory",
      activeWorkflow: "packaged-dc-drive",
      fellBack: false,
    });

    const failedDcDrive = resolveApplication(
      applicationInputs({ dcMotorStatus: "failed" }),
      "trajectory",
    );
    expect(failedDcDrive).toMatchObject({
      workspace: "relations",
      fellBack: true,
    });
  });

  it("resolves toolbar, navigation, and palette availability from the same facts", () => {
    const availability = resolveCommandAvailability(commandFacts());

    expect(Object.keys(availability)).toHaveLength(COMMAND_REGISTRY.length);
    expect(availability["workspace.geometry"]).toEqual({
      enabled: false,
      reason: "workflow.reason.cad-unavailable",
    });
    expect(availability["workspace.field"]).toEqual({
      enabled: false,
      reason: "command.reason.field-result-unavailable",
    });
    expect(availability["workspace.trajectory"]).toEqual({
      enabled: false,
      reason: "command.reason.trajectory-result-unavailable",
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
    expect(
      resolveCommandAvailability(commandFacts({ cylinderRunning: true }))["example.cylinder"],
    ).toEqual({
      enabled: false,
      reason: "command.reason.cylinder-running",
    });
    expect(
      resolveCommandAvailability(commandFacts({ dcMotorRunning: true }))["example.dc-drive"],
    ).toEqual({
      enabled: false,
      reason: "command.reason.dc-drive-running",
    });
    expect(
      resolveCommandAvailability(commandFacts({ structuralRunning: true }))["example.structural"],
    ).toEqual({
      enabled: false,
      reason: "command.reason.structural-running",
    });
    expect(
      resolveCommandAvailability(commandFacts({ structuralAvailable: true }))[
        "workspace.structure"
      ],
    ).toEqual({
      enabled: true,
      reason: null,
    });
  });

  it("scopes commands and focus targets to compatible current workflows", () => {
    const workflows = resolved(
      true,
      { status: "ready", acceptedModelDigest: DIGEST },
      "ready",
      "ready",
      "ready",
    );
    const relations = workflows[0]?.commands.map((command) => command.id);
    const spatial = workflows[1]?.commands.map((command) => command.id);
    const cylinder = workflows[2]?.commands.map((command) => command.id);
    const dcDrive = workflows[3]?.commands.map((command) => command.id);
    const structural = workflows[4]?.commands.map((command) => command.id);
    const cad = workflows[5]?.commands.map((command) => command.id);

    expect(relations).toContain("run.cancel");
    expect(spatial).toContain("run.cancel");
    expect(cylinder).toContain("focus.evidence");
    expect(cylinder).not.toContain("run.execute");
    expect(dcDrive).toContain("focus.evidence");
    expect(dcDrive).not.toContain("run.execute");
    expect(structural).toContain("focus.evidence");
    expect(structural).not.toContain("run.execute");
    expect(cad).toEqual([
      "model.compile",
      "history.undo",
      "history.redo",
      "workspace.relations",
      "workspace.geometry",
      "workspace.field",
      "workspace.trajectory",
      "workspace.structure",
      "example.cylinder",
      "example.dc-drive",
      "example.structural",
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

  it("routes evidence focus to the visible workflow-owned element", () => {
    expect(resolveElementFocusId("evidence-inspector", "relations", "relations")).toBe(
      "evidence-inspector",
    );
    expect(resolveElementFocusId("evidence-inspector", "scalar-elliptic", "relations")).toBe(
      "evidence-inspector",
    );
    expect(resolveElementFocusId("evidence-inspector", "cylinder-stokes", "field")).toBe(
      CYLINDER_EVIDENCE_FOCUS_ID,
    );
    expect(resolveElementFocusId("evidence-inspector", "packaged-dc-drive", "trajectory")).toBe(
      DC_MOTOR_EVIDENCE_FOCUS_ID,
    );
    expect(resolveElementFocusId("evidence-inspector", "structural-elasticity", "structure")).toBe(
      STRUCTURAL_EVIDENCE_FOCUS_ID,
    );
    expect(CYLINDER_EVIDENCE_FOCUS_ID).not.toBe("evidence-inspector");
    expect(DC_MOTOR_EVIDENCE_FOCUS_ID).not.toBe("evidence-inspector");
    expect(STRUCTURAL_EVIDENCE_FOCUS_ID).not.toBe("evidence-inspector");
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
    for (const workflow of resolveApplicationWorkflows(
      applicationInputs({ acceptedProjection: null }),
    )) {
      expect(workflow.availability.reason).not.toBeNull();
      if (workflow.availability.reason !== null) {
        expect(formatMessage(workflow.availability.reason)).not.toHaveLength(0);
      }
    }
  });
});
