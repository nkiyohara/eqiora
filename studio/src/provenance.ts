import type { RunEvidence } from "./reference-run-protocol";

/**
 * How much a displayed quantity is actually supported.
 *
 * RFC 0076 defines three states: verified, admissible, and stale. Only two are
 * reachable today, and this module deliberately does not fake the third.
 *
 * "Verified" there means *a registered case supports this exact capability
 * class*. Nothing a Run carries says that. `acceptance.independentVerifier` is
 * a narrower fact — whether a second numerical backend re-checked the solve —
 * and the wire pins it to `false`, so treating it as the verified signal would
 * have labelled every result in the product "unverified" while looking like it
 * had measured something. The evidence linkage is a gap in the owning contract,
 * reported as such rather than reconstructed here.
 */
export type EvidenceState = "accepted" | "stale";

/**
 * What the Run actually establishes.
 *
 * Derived from the record rather than asserted by the view: the inspector used
 * to print a fixed acceptance sentence that no data fed, so it would have kept
 * claiming no independent producer applied even after one did.
 */
export function evidenceState(_evidence: RunEvidence, stale: boolean): EvidenceState {
  return stale ? "stale" : "accepted";
}

/**
 * A non-color mark, because status carried by color alone survives neither a
 * screen reader nor a copied cell — which is exactly when misreading a number
 * costs most. Paired with a text label everywhere it appears.
 */
export function evidenceStateMark(state: EvidenceState): string {
  return state === "accepted" ? "✓" : "◌";
}

export function evidenceStateLabel(state: EvidenceState): string {
  return state === "accepted" ? "Accepted" : "Stale";
}

export function evidenceStateExplanation(state: EvidenceState): string {
  return state === "accepted"
    ? "This run was accepted by its semantic oracle, with an independently recomputed residual. Whether a registered case covers this capability class is not recorded in the run and is not shown."
    : "The model, run inputs, or revision changed after this result was produced.";
}

/**
 * The one segment of RFC 0076's provenance path that a Run cannot answer.
 *
 * Shown rather than omitted: an absent segment reads as verified, which is the
 * failure the contract exists to prevent.
 */
export const EVIDENCE_LINKAGE_UNAVAILABLE =
  "Not recorded in the run record; the registered-case link is a gap in the owning contract";

/**
 * The marking travels with the value.
 *
 * A quantity copied out of Studio, read from a table, or dropped into a
 * document carries its state and its unit in the text itself. Detached from
 * that, a number is indistinguishable from one that carries more support than
 * it does.
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
    verifier: acceptance.independentVerifier
      ? "Independently re-verified by a second backend"
      : "No second-backend re-verification",
  };
}
