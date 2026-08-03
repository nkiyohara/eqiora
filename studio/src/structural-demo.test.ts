import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  STRUCTURAL_CELL_COUNT,
  STRUCTURAL_CELLS_PER_AXIS,
  STRUCTURAL_DEMO_ID,
  STRUCTURAL_DEMO_PROTOCOL,
  STRUCTURAL_SCIENTIFIC_CASE,
  STRUCTURAL_VERTEX_COUNT,
  type StructuralDemoResult,
  structuralDemoResultSchema,
} from "./structural-demo-protocol";
import { StructuralDemoSession } from "./structural-demo-session";
import { StructuralDemoWorkspace } from "./structural-demo-workspace";

const digest = (character: string) => character.repeat(64);

function acceptedResult(runDigest = digest("c")): StructuralDemoResult {
  const vertices = Array.from({ length: STRUCTURAL_VERTEX_COUNT }, (_, index) => {
    const column = index % (STRUCTURAL_CELLS_PER_AXIS + 1);
    const row = Math.floor(index / (STRUCTURAL_CELLS_PER_AXIS + 1));
    return {
      index,
      coordinatesM: [column / STRUCTURAL_CELLS_PER_AXIS, row / STRUCTURAL_CELLS_PER_AXIS] as [
        number,
        number,
      ],
    };
  });
  const cells = Array.from({ length: STRUCTURAL_CELL_COUNT }, (_, index) => {
    const column = index % STRUCTURAL_CELLS_PER_AXIS;
    const row = Math.floor(index / STRUCTURAL_CELLS_PER_AXIS);
    const lowerLeft = row * (STRUCTURAL_CELLS_PER_AXIS + 1) + column;
    return {
      index,
      vertices: [
        lowerLeft,
        lowerLeft + 1,
        lowerLeft + STRUCTURAL_CELLS_PER_AXIS + 1,
        lowerLeft + STRUCTURAL_CELLS_PER_AXIS + 2,
      ] as [number, number, number, number],
    };
  });

  return structuralDemoResultSchema.parse({
    protocol: STRUCTURAL_DEMO_PROTOCOL,
    exampleId: STRUCTURAL_DEMO_ID,
    mesh: {
      spatialDimension: 2,
      cellsPerAxis: STRUCTURAL_CELLS_PER_AXIS,
      vertices,
      cells,
    },
    displacement: {
      unit: "m",
      valuesM: vertices.map(({ coordinatesM: [x, y] }) => [0.025 * x, 0.01 * x * y]),
    },
    balance: {
      unit: "N",
      constrainedReactionN: [-6, 0],
      integratedBodyForceN: [6, 0],
    },
    execution: {
      method: "continuous-galerkin",
      mesh: "generated-uniform-cartesian",
      space: "continuous-q1-two-component",
      quadrature: "gauss-legendre-2-per-axis",
      scalarType: "f64",
      placement: "one-host-one-worker",
      solver: "conjugate-gradient",
      preconditioner: "identity",
      reduction: "reproducible",
      convergenceReason: "residual-tolerance-satisfied",
      relativeTolerance: 1e-12,
      absoluteTolerance: 1e-14,
      completedIterations: 23,
      trueResidualNorm: 4e-13,
      residualTarget: 8e-13,
      assemblyPackets: 256,
      assemblyTargets: 578,
    },
    lineage: {
      modelDigest: digest("a"),
      realizationDigest: digest("b"),
      runDigest,
      semanticRevision: 11,
      realizationRevision: 1,
      outputArtifacts: 1,
    },
    evidence: {
      caseId: STRUCTURAL_SCIENTIFIC_CASE,
      status: "verified",
    },
  });
}

describe("mixed-boundary elasticity Studio protocol", () => {
  it("accepts the closed payload and rejects mesh, tuple, residual, and evidence drift", () => {
    const accepted = acceptedResult();
    expect(structuralDemoResultSchema.safeParse(accepted).success).toBe(true);

    const wrongOrder = structuredClone(accepted);
    const firstVertex = wrongOrder.mesh.vertices[0];
    if (firstVertex === undefined) throw new Error("fixture omitted vertex zero");
    firstVertex.index = 1;
    expect(structuralDemoResultSchema.safeParse(wrongOrder).success).toBe(false);

    const degenerate = structuredClone(accepted);
    const firstCell = degenerate.mesh.cells[0];
    if (firstCell === undefined) throw new Error("fixture omitted cell zero");
    firstCell.vertices[3] = firstCell.vertices[0];
    expect(structuralDemoResultSchema.safeParse(degenerate).success).toBe(false);

    const foreignExecution = {
      ...accepted,
      execution: { ...accepted.execution, scalarType: "f32" },
    };
    expect(structuralDemoResultSchema.safeParse(foreignExecution).success).toBe(false);

    const unacceptedResidual = {
      ...accepted,
      execution: {
        ...accepted.execution,
        trueResidualNorm: accepted.execution.residualTarget * 2,
      },
    };
    expect(structuralDemoResultSchema.safeParse(unacceptedResidual).success).toBe(false);

    const foreignCase = {
      ...accepted,
      evidence: { ...accepted.evidence, caseId: "solid.some-other-case" },
    };
    expect(structuralDemoResultSchema.safeParse(foreignCase).success).toBe(false);

    expect(structuralDemoResultSchema.safeParse({ ...accepted, recoveredStress: [] }).success).toBe(
      false,
    );
  });

  it("renders solver values, presentation scaling, attribution, and explicit non-claims", () => {
    const markup = renderToStaticMarkup(
      createElement(StructuralDemoWorkspace, { result: acceptedResult() }),
    );
    expect(markup).toContain("A clamped elastic panel, resolved");
    expect(markup).toContain("Display only: coordinates are drawn as x + scale × u.");
    expect(markup).toContain("Solver values and evidence below never change.");
    expect(markup).toContain(STRUCTURAL_SCIENTIFIC_CASE);
    expect(markup).toContain("registered case · verified");
    expect(markup).toContain("No stress, strain, traction, validation");
    expect(markup).toContain('id="structural-vertex-table"');
    expect(markup).toContain('id="structural-evidence-inspector"');
  });
});

describe("mixed-boundary elasticity Studio session", () => {
  it("publishes accepted native state and fails closed on diagnostic rejection", async () => {
    const accepted = acceptedResult();
    const transitions: string[] = [];
    const session = new StructuralDemoSession(
      {
        async runStructuralDemo(request) {
          expect(request).toEqual({ protocol: BRIDGE_PROTOCOL });
          return { protocol: BRIDGE_PROTOCOL, result: accepted, diagnostics: [] };
        },
      },
      (state) => transitions.push(state.kind),
    );
    expect(await session.run()).toEqual({ kind: "ready", result: accepted });
    expect(transitions).toEqual(["running", "ready"]);

    const rejected = new StructuralDemoSession({
      async runStructuralDemo() {
        return {
          protocol: BRIDGE_PROTOCOL,
          result: null,
          diagnostics: [
            {
              source: "studio",
              severity: "error",
              code: "studio.structural.native_required",
              message: "Native runtime required.",
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
      message: "Native runtime required.",
    });
  });

  it("discards a superseded response and clears without publishing stale evidence", async () => {
    let calls = 0;
    let releaseFirst: (result: StructuralDemoResult) => void = () => {
      throw new Error("first request was not started");
    };
    const firstResponse = new Promise<StructuralDemoResult>((resolve) => {
      releaseFirst = resolve;
    });
    const latest = acceptedResult(digest("d"));
    const session = new StructuralDemoSession({
      async runStructuralDemo() {
        calls += 1;
        const result = calls === 1 ? await firstResponse : latest;
        return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
      },
    });

    const stale = session.run();
    expect(await session.run()).toEqual({ kind: "ready", result: latest });
    releaseFirst(acceptedResult(digest("e")));
    await stale;
    expect(session.state).toEqual({ kind: "ready", result: latest });

    session.clear();
    expect(session.state).toEqual({ kind: "idle" });
  });
});
