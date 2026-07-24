import type { ZodType } from "zod";
import { BRIDGE_PROTOCOL, type BridgeEnvelope } from "./protocol";
import type { RunOutcome, RunRequest } from "./reference-run-protocol";
import type { SpatialRealizationRunRequest, SpatialRunResult } from "./spatial-protocol";

export function protocolFailure(message: string): BridgeEnvelope<never> {
  return {
    protocol: BRIDGE_PROTOCOL,
    result: null,
    diagnostics: [
      {
        source: "studio",
        severity: "error",
        code: "ST0002",
        message,
        graphPath: null,
        span: null,
      },
    ],
  };
}

export type RequestCheck<T> =
  | Readonly<{ ok: true; value: T }>
  | Readonly<{ ok: false; failure: BridgeEnvelope<never> }>;

export function checkedRequest<T>(
  schema: ZodType<T>,
  request: unknown,
  operation: string,
): RequestCheck<T> {
  const decoded = schema.safeParse(request);
  if (!decoded.success) {
    return {
      ok: false,
      failure: protocolFailure(
        `${operation} request does not satisfy the current Studio bridge protocol.`,
      ),
    };
  }
  return { ok: true, value: decoded.data };
}

export function outcomeMatchesRunRequest(request: RunRequest, outcome: RunOutcome): boolean {
  const plan =
    outcome.kind === "completed" ? outcome.result.evidence.plan : outcome.cancellation.plan;
  const identityMatches =
    plan.key === request.planKey &&
    plan.integration.endTime === request.endTime &&
    plan.integration.maxStep === request.maxStep;
  return outcome.kind === "completed"
    ? identityMatches && outcome.result.digest === request.digest
    : identityMatches && outcome.cancellation.runId === request.runId;
}

export function spatialResultMatchesRequest(
  request: SpatialRealizationRunRequest,
  result: SpatialRunResult,
): boolean {
  return (
    result.runId === request.runId &&
    result.digest === request.digest &&
    result.plan.key === request.planKey &&
    result.plan.modelDigest === request.digest &&
    result.plan.realizationRevision === request.realizationRevision &&
    result.plan.discretization.method === request.method &&
    result.plan.discretization.cellsPerAxis === request.cellsPerAxis &&
    result.plan.placement.workers === request.workers
  );
}
