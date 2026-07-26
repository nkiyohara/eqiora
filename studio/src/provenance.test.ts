import { describe, expect, it } from "vitest";
import {
  acceptanceSummary,
  evidenceState,
  evidenceStateExplanation,
  evidenceStateLabel,
  evidenceStateMark,
  markedQuantity,
} from "./provenance";
import type { RunEvidence } from "./reference-run-protocol";

function evidence(independentVerifier: boolean): RunEvidence {
  return {
    plan: {
      key: "plan-key",
      adapter: { id: "reference", version: "0.1.0" },
      placement: { workers: 1 },
      integrator: {
        method: "backward-euler",
        initialStep: 1e-3,
        minimumStep: 1e-9,
        maximumStep: 1e-1,
      },
      nonlinear: {
        method: "dense-finite-difference-newton",
        absoluteTolerance: 1e-10,
        relativeTolerance: 1e-8,
        maximumIterations: 20,
      },
      events: {
        timeTolerance: 1e-9,
        guardTolerance: 1e-9,
        maximumLocalizationIterations: 40,
        maximumZeroTimeEvents: 8,
      },
      limits: { maximumSteps: 100_000 },
      acceptance: { kind: "semantic-oracle", independentVerifier },
    },
    elapsedSeconds: 0.25,
    fieldCount: 1,
    sampleCount: 100,
  } as unknown as RunEvidence;
}

describe("evidence state", () => {
  it("reads acceptance from the run record rather than asserting it", () => {
    expect(evidenceState(evidence(false), false)).toBe("admissible");
    expect(evidenceState(evidence(true), false)).toBe("verified");
  });

  it("lets staleness win over acceptance, because a moved input invalidates both", () => {
    expect(evidenceState(evidence(true), true)).toBe("stale");
    expect(evidenceState(evidence(false), true)).toBe("stale");
  });

  it("never renders an unverified result the same way as a verified one", () => {
    const states = ["verified", "admissible", "stale"] as const;
    const labels = states.map(evidenceStateLabel);
    const marks = states.map(evidenceStateMark);
    expect(new Set(labels).size).toBe(states.length);
    expect(new Set(marks).size).toBe(states.length);
  });

  it("says plainly that an admissible result is not evidence of correctness", () => {
    expect(evidenceStateExplanation("admissible")).toContain("not evidence of correctness");
  });
});

describe("marking travels with the value", () => {
  it("carries state and unit in the text itself, so a copied number stays honest", () => {
    const text = markedQuantity(1.23456789, "m / s", "admissible");
    expect(text).toContain("[m / s]");
    expect(text).toContain("Admissible, unverified");
  });

  it("omits the bracket only when the quantity is genuinely dimensionless", () => {
    expect(markedQuantity(2, "", "verified")).not.toContain("[");
    expect(markedQuantity(2, "  ", "verified")).not.toContain("[");
  });

  it("keeps the state visible for a verified value too, so absence is never the signal", () => {
    expect(markedQuantity(1, "1", "verified")).toContain("Verified");
  });
});

describe("acceptance summary", () => {
  it("reports the verifier from the plan, not from a fixed sentence", () => {
    expect(acceptanceSummary(evidence(false)).verifier).toBe("No independent verifier");
    expect(acceptanceSummary(evidence(true)).verifier).toBe("Independently verified");
  });
});
