import { describe, expect, it } from "vitest";
import { cadProjectionMatchesRequest } from "./cad-bridge";

describe("CAD projection response binding", () => {
  it("accepts only the exact requested Model digest", () => {
    const request = { modelDigest: "1".repeat(64) };

    expect(cadProjectionMatchesRequest(request, { modelDigest: "1".repeat(64) })).toBe(true);
    expect(cadProjectionMatchesRequest(request, { modelDigest: "2".repeat(64) })).toBe(false);
  });
});
