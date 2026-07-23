import { describe, expect, it } from "vitest";
import { validateValueEditInput } from "./value-edit";

describe("typed value edit input", () => {
  it("admits one distinct finite coherent-SI scalar", () => {
    expect(validateValueEditInput("2.5e-3", 1)).toEqual({ value: 0.0025, error: null });
  });

  it("keeps the unchanged revision value neutral", () => {
    expect(validateValueEditInput("1", 1)).toEqual({ value: null, error: null });
  });

  it("rejects empty, non-finite, and resource-excessive input", () => {
    for (const input of ["", "NaN", "Infinity", "9".repeat(129)]) {
      expect(validateValueEditInput(input, 1).value).toBeNull();
      expect(validateValueEditInput(input, 1).error).not.toBeNull();
    }
  });
});
