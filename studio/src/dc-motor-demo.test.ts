import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { describe, expect, it } from "vitest";
import {
  DC_MOTOR_DEMO_ID,
  DC_MOTOR_DEMO_PROTOCOL,
  DC_MOTOR_SCIENTIFIC_CASE,
  type DcMotorDemoResult,
  dcMotorDemoResultSchema,
} from "./dc-motor-demo-protocol";
import { DcMotorDemoSession } from "./dc-motor-demo-session";
import { DcMotorDemoWorkspace } from "./dc-motor-demo-workspace";
import { BRIDGE_PROTOCOL } from "./protocol";

const digest = (character: string) => character.repeat(64);

function acceptedResult(runDigest = digest("a")): DcMotorDemoResult {
  const samples = Array.from({ length: 101 }, (_, step) => ({
    step,
    timeS: step * 0.001,
    currentA: step * 0.01,
    angularSpeedPerS: step * 0.001,
    heldVoltageV: 10 - Math.floor(step / 10) * 0.1,
  }));
  return dcMotorDemoResultSchema.parse({
    protocol: DC_MOTOR_DEMO_PROTOCOL,
    exampleId: DC_MOTOR_DEMO_ID,
    trajectory: {
      samples,
      commits: Array.from({ length: 11 }, (_, ordinal) => {
        const sample = samples[ordinal * 10];
        if (sample === undefined) throw new Error("fixture omitted a commit boundary");
        return {
          step: sample.step,
          timeS: sample.timeS,
          heldVoltageV: sample.heldVoltageV,
        };
      }),
      units: {
        current: "A",
        angularSpeed: "s^-1",
        heldVoltage: "V",
        time: "s",
      },
    },
    packageGraph: {
      root: "org.example.dc_motor_control@0.1.0",
      resolutionDigest: digest("1"),
      nodes: [
        {
          name: "Eqiora.Electrical.Basic",
          version: "0.1.0",
          semanticDigest: digest("2"),
          sourceDigest: digest("3"),
        },
        {
          name: "Eqiora.Electromechanical.DcDrive",
          version: "0.1.0",
          semanticDigest: digest("4"),
          sourceDigest: digest("5"),
        },
        {
          name: "org.example.dc_motor_control",
          version: "0.1.0",
          semanticDigest: digest("6"),
          sourceDigest: digest("7"),
        },
      ],
      edges: [
        {
          declaring: "Eqiora.Electromechanical.DcDrive@0.1.0",
          alias: "electrical",
          target: "Eqiora.Electrical.Basic@0.1.0",
        },
        {
          declaring: "org.example.dc_motor_control@0.1.0",
          alias: "drive",
          target: "Eqiora.Electromechanical.DcDrive@0.1.0",
        },
        {
          declaring: "org.example.dc_motor_control@0.1.0",
          alias: "electrical",
          target: "Eqiora.Electrical.Basic@0.1.0",
        },
      ],
    },
    execution: {
      method: "backward-euler",
      scalarType: "f64",
      placement: "one-host-one-worker",
      endTimeS: 0.1,
      maximumStepS: 0.001,
      samplePeriodS: 0.01,
      acceptedSteps: 100,
      holdIntervals: 10,
      controllerCommits: 11,
    },
    lineage: {
      modelDigest: digest("8"),
      compilationDigest: digest("9"),
      runDigest,
      runBindingDigest: digest("b"),
      semanticRevision: 17,
    },
    evidence: {
      caseId: DC_MOTOR_SCIENTIFIC_CASE,
      status: "historical-a3-only",
      physicalPortSamplesPresented: false,
    },
  });
}

describe("packaged DC-drive Studio protocol", () => {
  it("accepts one closed payload and rejects structural trajectory drift", () => {
    const accepted = acceptedResult();
    expect(dcMotorDemoResultSchema.safeParse(accepted).success).toBe(true);

    const perStepVoltage = structuredClone(accepted);
    const mutatedSample = perStepVoltage.trajectory.samples[5];
    if (mutatedSample === undefined) throw new Error("fixture omitted step 5");
    mutatedSample.heldVoltageV -= 0.25;
    expect(dcMotorDemoResultSchema.safeParse(perStepVoltage).success).toBe(false);

    const offByOneCommit = structuredClone(accepted);
    const mutatedCommit = offByOneCommit.trajectory.commits[3];
    if (mutatedCommit === undefined) throw new Error("fixture omitted commit 3");
    mutatedCommit.step = 31;
    expect(dcMotorDemoResultSchema.safeParse(offByOneCommit).success).toBe(false);

    const foreignEdge = structuredClone(accepted);
    const mutatedEdge = foreignEdge.packageGraph.edges[0];
    if (mutatedEdge === undefined) throw new Error("fixture omitted package edge");
    mutatedEdge.target = "org.example.dc_motor_control@0.1.0";
    expect(dcMotorDemoResultSchema.safeParse(foreignEdge).success).toBe(false);
  });

  it("renders attribution separately from the three production series", () => {
    const markup = renderToStaticMarkup(
      createElement(DcMotorDemoWorkspace, { result: acceptedResult() }),
    );
    expect(markup).toContain("Production trajectory");
    expect(markup).toContain("No quantity on this view is recomputed by the application.");
    expect(markup).toContain(`Registered case <code>${DC_MOTOR_SCIENTIFIC_CASE}</code>`);
    expect(markup).toContain("current release lineage · unverified");
    expect(markup).toContain("historical a3 evidence only");
    expect(markup).toContain('class="dc-drive-chart__line voltage"');
    expect(markup).not.toContain("Copper loss");
    expect(markup).not.toContain("Stored energy");
    expect(markup).not.toContain("Numerical dissipation");
  });
});

describe("packaged DC-drive Studio session", () => {
  it("publishes accepted native state and fails closed on diagnostic rejection", async () => {
    const accepted = acceptedResult();
    const transitions: string[] = [];
    const session = new DcMotorDemoSession(
      {
        async runDcMotorDemo(request) {
          expect(request).toEqual({ protocol: BRIDGE_PROTOCOL });
          return { protocol: BRIDGE_PROTOCOL, result: accepted, diagnostics: [] };
        },
      },
      (state) => transitions.push(state.kind),
    );
    expect(await session.run()).toEqual({ kind: "ready", result: accepted });
    expect(transitions).toEqual(["running", "ready"]);

    const rejected = new DcMotorDemoSession({
      async runDcMotorDemo() {
        return {
          protocol: BRIDGE_PROTOCOL,
          result: null,
          diagnostics: [
            {
              source: "studio",
              severity: "error",
              code: "ST0003",
              message: "Package lineage was rejected.",
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
      message: "Package lineage was rejected.",
    });
  });

  it("discards a superseded response", async () => {
    let calls = 0;
    let releaseFirst: (result: DcMotorDemoResult) => void = () => {
      throw new Error("first request was not started");
    };
    const firstResponse = new Promise<DcMotorDemoResult>((resolve) => {
      releaseFirst = resolve;
    });
    const latest = acceptedResult(digest("c"));
    const session = new DcMotorDemoSession({
      async runDcMotorDemo() {
        calls += 1;
        const result = calls === 1 ? await firstResponse : latest;
        return { protocol: BRIDGE_PROTOCOL, result, diagnostics: [] };
      },
    });

    const stale = session.run();
    expect(await session.run()).toEqual({ kind: "ready", result: latest });
    releaseFirst(acceptedResult(digest("d")));
    await stale;
    expect(session.state).toEqual({ kind: "ready", result: latest });
  });
});
