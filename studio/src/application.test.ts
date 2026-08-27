import { describe, expect, test } from "vitest";
import {
  COMMAND_REGISTRY,
  resolveApplication,
  resolveCommandAvailability,
  WORKFLOW_REGISTRY,
} from "./application";
import { BRIDGE_PROTOCOL, type DocumentProjection } from "./protocol";

const document: DocumentProjection = {
  protocol: BRIDGE_PROTOCOL,
  digest: "0123456789abcdef",
  revision: 1,
  modelId: "Model:test",
  nodes: [],
  edges: [],
};
const inputs = {
  acceptedProjection: document,
  cad: { status: "ready" as const, acceptedModelDigest: document.digest },
  dcMotorStatus: "idle" as const,
};
describe("retained Studio application registry", () => {
  test("contains only compiler, packaged DC, and CAD workflows", () =>
    expect(WORKFLOW_REGISTRY.map((item) => item.id)).toEqual([
      "relations",
      "packaged-dc-drive",
      "cad-box",
      "cad-authored",
    ]));
  test("contains no retired execution commands", () =>
    expect(COMMAND_REGISTRY.map((item) => item.id)).not.toContain("run.execute"));
  test("binds CAD availability to the compiled digest", () => {
    expect(resolveApplication(inputs, "geometry").workspace).toBe("geometry");
    expect(
      resolveApplication(
        { ...inputs, cad: { status: "ready", acceptedModelDigest: "foreign-digest-00" } },
        "geometry",
      ).workspace,
    ).toBe("relations");
  });
  test("opens packaged DC only after its accepted result exists", () => {
    expect(resolveApplication(inputs, "trajectory").workspace).toBe("relations");
    expect(resolveApplication({ ...inputs, dcMotorStatus: "ready" }, "trajectory").workspace).toBe(
      "trajectory",
    );
  });
  test("retains compile and CAD authoring commands", () => {
    const availability = resolveCommandAvailability({
      activeWorkflow: "relations",
      compiling: false,
      documentAccepted: true,
      valueEditReady: false,
      valueEditBlock: null,
      revisionNavigationBlocked: false,
      canUndo: false,
      canRedo: false,
      selectedEntity: false,
      evidenceAvailable: false,
      trajectoryAvailable: false,
      dcMotorRunning: false,
      cadAvailability: { kind: "available", reason: null },
    });
    expect(availability["model.compile"].enabled).toBe(true);
    expect(availability["workspace.cad-authoring"].enabled).toBe(true);
  });
});
