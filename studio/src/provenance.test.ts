import { describe, expect, it } from "vitest";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  acceptanceSummary,
  EVIDENCE_LINKAGE_UNAVAILABLE,
  evidenceState,
  evidenceStateExplanation,
  evidenceStateLabel,
  evidenceStateMark,
  markedQuantity,
} from "./provenance";
import { type RunEvidence, runEvidenceSchema } from "./reference-run-protocol";

/**
 * Parsed through the real schema rather than cast into shape. An earlier
 * version of this suite built the "verified" case with `as unknown as
 * RunEvidence`, which proved a branch that the wire cannot produce.
 */
function evidence(): RunEvidence {
  return runEvidenceSchema.parse({
    plan: {
      protocol: BRIDGE_PROTOCOL,
      key: "plan-key",
      adapter: { id: "reference", version: "0.1.0" },
      placement: { kind: "host", workers: 1 },
      integration: {
        method: "backward-euler",
        endTime: 1,
        maxStep: 1e-1,
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
      acceptance: { kind: "semantic-oracle", independentVerifier: false },
    },
    elapsedSeconds: 0.25,
    fieldCount: 1,
    sampleCount: 100,
  });
}

describe("evidence state", () => {
  it("distinguishes only what a run record can establish", () => {
    expect(evidenceState(evidence(), false)).toBe("accepted");
    expect(evidenceState(evidence(), true)).toBe("stale");
  });

  it("never renders a stale result the same way as a current one", () => {
    expect(evidenceStateLabel("accepted")).not.toBe(evidenceStateLabel("stale"));
    expect(evidenceStateMark("accepted")).not.toBe(evidenceStateMark("stale"));
  });

  it("says plainly that registered-case coverage is not shown", () => {
    expect(evidenceStateExplanation("accepted")).toContain("is not shown");
  });

  it("names the missing provenance segment rather than omitting it", () => {
    expect(EVIDENCE_LINKAGE_UNAVAILABLE).toContain("gap in the owning contract");
  });
});

describe("marking travels with the value", () => {
  it("carries state and unit in the text itself, so a copied number stays honest", () => {
    const text = markedQuantity(1.23456789, "m / s", "accepted");
    expect(text).toContain("[m / s]");
    expect(text).toContain("Accepted");
  });

  it("omits the bracket only when the quantity is genuinely dimensionless", () => {
    expect(markedQuantity(2, "", "accepted")).not.toContain("[");
    expect(markedQuantity(2, "  ", "accepted")).not.toContain("[");
  });

  it("keeps the state visible for a current value too, so absence is never the signal", () => {
    expect(markedQuantity(1, "1", "accepted")).toContain("Accepted");
  });
});

describe("acceptance summary", () => {
  it("reports second-backend re-verification from the plan, not from a fixed sentence", () => {
    // The wire pins this to false today; the accessor must still read it rather
    // than restate it, so the view does not assert what it has not read.
    expect(acceptanceSummary(evidence()).verifier).toBe("No second-backend re-verification");
    expect(acceptanceSummary(evidence()).kind).toBe("Semantic oracle");
  });
});
