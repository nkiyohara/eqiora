import type { ZodType } from "zod";
import { BRIDGE_PROTOCOL, type BridgeEnvelope } from "./protocol";

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
