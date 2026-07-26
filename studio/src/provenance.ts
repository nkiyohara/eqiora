import type { RunEvidence } from "./reference-run-protocol";

/**
 * How much a displayed quantity is actually supported.
 *
 * RFC 0076 requires these three never to collapse into each other. A result
 * that no independent verifier accepted must not render like one that was, and
 * a result whose inputs have moved must not render like a current one.
 */
export type EvidenceState = "verified" | "admissible" | "stale";

/**
 * Derived from the run record rather than asserted by the view.
 *
 * The previous inspector printed a fixed acceptance sentence that no data fed,
 * so it would have kept claiming no independent producer applied even after one
 * did. A view must not assert what it has not read.
 */
export function evidenceState(evidence: RunEvidence, stale: boolean): EvidenceState {
  if (stale) return "stale";
  return evidence.plan.acceptance.independentVerifier ? "verified" : "admissible";
}

/**
 * A non-color mark, because status carried by color alone survives neither a
 * screen reader nor a copied cell — which is exactly when misreading a number
 * costs most. Paired with a text label everywhere it appears.
 */
export function evidenceStateMark(state: EvidenceState): string {
  switch (state) {
    case "verified":
      return "✓";
    case "admissible":
      return "△";
    case "stale":
      return "◌";
  }
}

export function evidenceStateLabel(state: EvidenceState): string {
  switch (state) {
    case "verified":
      return "Verified";
    case "admissible":
      return "Admissible, unverified";
    case "stale":
      return "Stale";
  }
}

/**
 * What the state means for the number next to it, in the user's terms rather
 * than the pipeline's.
 */
export function evidenceStateExplanation(state: EvidenceState): string {
  switch (state) {
    case "verified":
      return "An independent verifier accepted this result.";
    case "admissible":
      return "The configuration was admitted and executed, but no independent verifier accepted this result. It is not evidence of correctness.";
    case "stale":
      return "The model, run inputs, or revision changed after this result was produced.";
  }
}

/**
 * The marking travels with the value.
 *
 * A quantity copied out of Studio, dropped into a table, or read from a legend
 * carries its state and its unit in the text itself. Detached from that, a
 * number is indistinguishable from a verified one, which is the failure this
 * whole contract exists to prevent.
 */
export function markedQuantity(
  value: number,
  dimension: string,
  state: EvidenceState,
  significantDigits = 6,
): string {
  const unit = dimension.trim() === "" ? "" : ` [${dimension}]`;
  return `${value.toPrecision(significantDigits)}${unit} (${evidenceStateLabel(state)})`;
}

/**
 * The acceptance line for the inspector, read from the plan.
 */
export function acceptanceSummary(evidence: RunEvidence): {
  readonly kind: string;
  readonly verifier: string;
} {
  const { acceptance } = evidence.plan;
  return {
    kind: acceptance.kind === "semantic-oracle" ? "Semantic oracle" : acceptance.kind,
    verifier: acceptance.independentVerifier ? "Independently verified" : "No independent verifier",
  };
}
