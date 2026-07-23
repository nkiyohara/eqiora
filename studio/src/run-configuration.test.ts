import { describe, expect, it } from "vitest";
import { validateRunConfiguration } from "./run-configuration";

describe("run configuration", () => {
  it("accepts positive decimal and scientific notation", () => {
    expect(validateRunConfiguration({ endTime: " 4.0 ", maxStep: "1e-1" })).toEqual({
      value: { endTime: 4, maxStep: 0.1 },
      errors: { endTime: null, maxStep: null },
    });
  });

  it.each(["", " ", "0", "-1", "Infinity", "NaN", "1 second"])(
    "rejects an invalid editable value: %s",
    (endTime) => {
      const validation = validateRunConfiguration({ endTime, maxStep: "0.1" });
      expect(validation.value).toBeNull();
      expect(validation.errors.endTime).not.toBeNull();
    },
  );

  it("rejects requests beyond the bridge step limit", () => {
    const validation = validateRunConfiguration({ endTime: "10", maxStep: "1e-7" });
    expect(validation.value).toBeNull();
    expect(validation.errors.maxStep).toContain("5,000,000");
  });
});
