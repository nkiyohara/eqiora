import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  FSI_DEMO_ID,
  FSI_DEMO_PROTOCOL,
  FSI_STEP_CASE,
  FSI_TRAJECTORY_CASE,
  type FsiDemoResult,
  fsiDemoResultSchema,
} from "./fsi-demo-protocol";
import { FsiDemoSession } from "./fsi-demo-session";
import { FsiDemoWorkspace } from "./fsi-demo-workspace";
import { BRIDGE_PROTOCOL } from "./protocol";

const digest = (character: string) => character.repeat(64);

const vertices = [
  [0, 0],
  [1, 0],
  [0, 0.5],
  [1, 0.5],
  [0, 1],
  [1, 1],
  [2, 0],
  [2, 0.5],
  [2, 1],
] as const;

const cells = [
  [0, 1, 3],
  [0, 3, 2],
  [2, 3, 5],
  [2, 5, 4],
  [1, 6, 7],
  [1, 7, 3],
  [3, 7, 8],
  [3, 8, 5],
] as const;

function acceptedResult(runDigest = digest("f")): FsiDemoResult {
  const step = (ordinal: 1 | 2) => ({
    step: ordinal,
    timeS: ordinal * 0.05,
    velocity: {
      unit: "m/s",
      values: vertices.map((_, vertex) => [0.001 * ordinal * vertex, -0.0002 * vertex]),
    },
    fluidBubbleVelocity: {
      unit: "m/s",
      values: Array.from({ length: 4 }, (_, cell) => [0.0001 * ordinal * (cell + 1), 0]),
    },
    pressure: {
      unit: "kg m^-1 s^-2",
      supportVertices: [0, 1, 2, 3, 4, 5],
      values: [0.1, 0.08, 0.04, 0.02, -0.01, -0.03].map((value) => value / ordinal),
    },
    displacement: {
      unit: "m",
      values: vertices.map((_, vertex) =>
        [0, 2, 4].includes(vertex)
          ? [0, 0]
          : [0.02 - 0.002 * ordinal + 0.0001 * vertex, 0.00005 * ordinal * vertex],
      ),
    },
    interfaceActions: [
      {
        vertex: 3,
        unit: "N/m",
        fluid: [0.03 / ordinal, -0.01 / ordinal],
        solid: [-0.03 / ordinal, 0.01 / ordinal],
        imbalance: [0, 0],
      },
    ],
    energy: {
      unit: "J/m",
      previousKinetic: 0.001,
      nextKinetic: 0.0008,
      previousElastic: 0.002,
      nextElastic: 0.0018,
      kineticIncrement: 0.0001,
      elasticIncrement: 0.0001,
      viscousDissipation: 0.0002,
      defect: 2e-19,
    },
    physicsAcceptance: {
      numericalResidualNorm: 2e-17,
      continuityResidualNorm: 1e-17,
      kinematicResidualNorm: 1e-18,
      interfaceVelocityJumpNorm: 0,
      interfaceActionImbalanceNPerM: 2e-16,
      absoluteEnergyDefectJPerM: 2e-19,
    },
    solverStopping: {
      convergenceReason: "residual-tolerance-satisfied",
      completedIterations: 16,
      trueResidualNorm: 2e-17,
      residualTarget: 1e-12,
    },
    assembly: {
      packetCount: 8,
      targetCount: 16,
    },
  });
  return fsiDemoResultSchema.parse({
    protocol: FSI_DEMO_PROTOCOL,
    exampleId: FSI_DEMO_ID,
    mesh: {
      vertices: vertices.map((coordinatesM, index) => ({ index, coordinatesM })),
      cells: cells.map((cellVertices, index) => ({
        index,
        vertices: cellVertices,
        region: index < 4 ? "fluid" : "solid",
      })),
      interfaceFacets: [
        { index: 2, vertices: [1, 3] },
        { index: 6, vertices: [3, 5] },
      ],
    },
    steps: [step(1), step(2)],
    execution: {
      method: "fixed-reference-monolithic-fsi",
      fluidSpace: "continuous-mini-velocity-p1-pressure",
      solidSpace: "continuous-p1-velocity-displacement",
      timeMethod: "backward-euler",
      timeStepS: 0.05,
      lengthScaleM: 2,
      velocityScaleMPerS: 0.5,
      pressureScalePa: 4,
      scalarType: "f64",
      placement: "one-host-one-worker",
      solver: "minimum-residual",
      preconditioner: "identity",
      reduction: "reproducible",
      relativeTolerance: 1e-11,
      absoluteTolerance: 1e-13,
    },
    lineage: {
      modelDigest: digest("a"),
      geometryDigest: digest("b"),
      correspondenceDigest: digest("c"),
      meshDigest: digest("d"),
      realizationDigest: digest("e"),
      runDigest,
      stateDigests: [digest("1"), digest("2")],
      trajectoryDigest: digest("3"),
      semanticRevision: 17,
      realizationRevision: 1,
      runOutputArtifacts: 1,
    },
    evidence: [
      { caseId: FSI_STEP_CASE, status: "verified" },
      { caseId: FSI_TRAJECTORY_CASE, status: "verified" },
    ],
  });
}

describe("fixed-reference FSI Studio protocol", () => {
  it("accepts one closed two-step payload and rejects structural or acceptance drift", () => {
    const accepted = acceptedResult();
    expect(fsiDemoResultSchema.safeParse(accepted).success).toBe(true);

    const wrongCell = structuredClone(accepted);
    const firstCell = wrongCell.mesh.cells[0];
    if (firstCell === undefined) throw new Error("fixture omitted first FSI cell");
    firstCell.vertices = [0, 3, 1];
    expect(fsiDemoResultSchema.safeParse(wrongCell).success).toBe(false);

    const wrongSupport = structuredClone(accepted);
    const firstStep = wrongSupport.steps[0];
    if (firstStep === undefined) throw new Error("fixture omitted first FSI step");
    firstStep.pressure.supportVertices[5] = 6;
    expect(fsiDemoResultSchema.safeParse(wrongSupport).success).toBe(false);

    const escapedDisplacement = structuredClone(accepted);
    const escapedStep = escapedDisplacement.steps[0];
    if (escapedStep === undefined) throw new Error("fixture omitted first FSI step");
    escapedStep.displacement.values[0] = [1e-3, 0];
    expect(fsiDemoResultSchema.safeParse(escapedDisplacement).success).toBe(false);

    const unaccepted = structuredClone(accepted);
    const unacceptedStep = unaccepted.steps[0];
    if (unacceptedStep === undefined) throw new Error("fixture omitted first FSI step");
    unacceptedStep.physicsAcceptance.interfaceActionImbalanceNPerM = 1e-8;
    expect(fsiDemoResultSchema.safeParse(unaccepted).success).toBe(false);

    const duplicate = structuredClone(accepted);
    const duplicateFirst = duplicate.steps[0];
    const duplicateSecond = duplicate.steps[1];
    if (duplicateFirst === undefined || duplicateSecond === undefined) {
      throw new Error("fixture omitted one frozen FSI step");
    }
    duplicateSecond.displacement.values = structuredClone(duplicateFirst.displacement.values);
    expect(fsiDemoResultSchema.safeParse(duplicate).success).toBe(false);

    expect(fsiDemoResultSchema.safeParse({ ...accepted, stress: [] }).success).toBe(false);
  });

  it("renders separate solver and physics evidence, exact units, lineage, and non-claims", () => {
    const markup = renderToStaticMarkup(
      createElement(FsiDemoWorkspace, { result: acceptedResult() }),
    );
    expect(markup).toContain("One trace. Two bodies.");
    expect(markup).toContain("Physics closes");
    expect(markup).toContain("MINRES report");
    expect(markup).toContain("N/m");
    expect(markup).toContain("J/m · intrinsic 2D");
    expect(markup).toContain("solid display ×12");
    expect(markup).toContain(FSI_STEP_CASE);
    expect(markup).toContain(FSI_TRAJECTORY_CASE);
    expect(markup).toContain("No ALE motion, advection, remeshing");
    expect(markup).toContain('id="fsi-vertex-table"');
    expect(markup).toContain('id="fsi-evidence-inspector"');
  });
});

describe("fixed-reference FSI Studio session", () => {
  it("publishes accepted state and fails closed on native refusal", async () => {
    const accepted = acceptedResult();
    const transitions: string[] = [];
    const session = new FsiDemoSession(
      {
        async runFsiDemo(request) {
          expect(request).toEqual({ protocol: BRIDGE_PROTOCOL });
          return { protocol: BRIDGE_PROTOCOL, result: accepted, diagnostics: [] };
        },
      },
      (state) => transitions.push(state.kind),
    );
    expect(await session.run()).toEqual({ kind: "ready", result: accepted });
    expect(transitions).toEqual(["running", "ready"]);

    const rejected = new FsiDemoSession({
      async runFsiDemo() {
        return {
          protocol: BRIDGE_PROTOCOL,
          result: null,
          diagnostics: [
            {
              source: "studio",
              severity: "error",
              code: "studio.fsi.native_required",
              message: "Native FSI runtime required.",
              graphPath: [],
              span: null,
              patch: null,
            },
          ],
        };
      },
    });
    expect(await rejected.run()).toMatchObject({
      kind: "failed",
      message: "Native FSI runtime required.",
    });
  });

  it("discards superseded native evidence", async () => {
    let calls = 0;
    let releaseFirst: (result: FsiDemoResult) => void = () => {
      throw new Error("first FSI request was not started");
    };
    const firstResponse = new Promise<FsiDemoResult>((resolve) => {
      releaseFirst = resolve;
    });
    const latest = acceptedResult(digest("8"));
    const session = new FsiDemoSession({
      async runFsiDemo() {
        calls += 1;
        const result = calls === 1 ? await firstResponse : latest;
        return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
      },
    });

    const stale = session.run();
    expect(await session.run()).toEqual({ kind: "ready", result: latest });
    releaseFirst(acceptedResult(digest("9")));
    await stale;
    expect(session.state).toEqual({ kind: "ready", result: latest });
  });
});
