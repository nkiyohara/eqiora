import { describe, expect, test } from "vitest";
import { z } from "zod";
import { checkedRequest, protocolFailure } from "./bridge-contract";
import { BRIDGE_PROTOCOL } from "./protocol";
import { valueEditPlanSchema } from "./value-edit-protocol";

describe("bridge contract", () => {
  test("accepts only the declared schema", () => {
    expect(checkedRequest(z.object({ id: z.string() }), { id: "x" }, "test").ok).toBe(true);
    expect(checkedRequest(z.object({ id: z.string() }), { id: 1 }, "test").ok).toBe(false);
  });
  test("returns a diagnostic-only failure", () => {
    const failure = protocolFailure("bad");
    expect(failure.result).toBeNull();
    expect(failure.diagnostics[0]?.code).toBe("ST0002");
  });
  test("does not let a value edit change physical dimension", () => {
    const decoded = valueEditPlanSchema.safeParse({
      protocol: BRIDGE_PROTOCOL,
      key: `eqiora.value-edit-plan/v1:${"a".repeat(64)}`,
      baseDigest: "0123456789abcdef",
      baseRevision: 1,
      targetId: "Parameter:x",
      before: { value: 1, dimension: "T^-1" },
      after: { value: 2, dimension: "L" },
      transactionDigest: "a".repeat(64),
    });
    expect(decoded.success).toBe(false);
  });
});
