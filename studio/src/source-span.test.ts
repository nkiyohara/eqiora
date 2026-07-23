import { describe, expect, it } from "vitest";
import { sourceLocationLabel, sourceSelection, utf8ByteOffsetToUtf16Index } from "./source-span";

describe("source span projection", () => {
  const source = "a😀βz";

  it("converts UTF-8 byte boundaries to DOM UTF-16 indices", () => {
    expect(utf8ByteOffsetToUtf16Index(source, 0)).toBe(0);
    expect(utf8ByteOffsetToUtf16Index(source, 1)).toBe(1);
    expect(utf8ByteOffsetToUtf16Index(source, 5)).toBe(3);
    expect(utf8ByteOffsetToUtf16Index(source, 7)).toBe(4);
    expect(utf8ByteOffsetToUtf16Index(source, 8)).toBe(5);
  });

  it("clamps malformed interior and out-of-range byte offsets safely", () => {
    expect(utf8ByteOffsetToUtf16Index(source, 3)).toBe(1);
    expect(utf8ByteOffsetToUtf16Index(source, 99)).toBe(source.length);
  });

  it("selects Unicode spans and reports human source locations", () => {
    const span = { file: "model.eqi", start: 1, end: 7 };
    expect(sourceSelection(source, span)).toEqual({ start: 1, end: 4 });
    expect(sourceLocationLabel(`header\n${source}`, { ...span, start: 7, end: 14 })).toBe(
      "model.eqi:2:1",
    );
  });
});
