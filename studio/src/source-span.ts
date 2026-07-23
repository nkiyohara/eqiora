import type { SourceSpan } from "./protocol";

export interface SourceSelection {
  readonly start: number;
  readonly end: number;
}

function utf8ScalarWidth(scalar: string): number {
  const codePoint = scalar.codePointAt(0) ?? 0;
  if (codePoint <= 0x7f) return 1;
  if (codePoint <= 0x7ff) return 2;
  if (codePoint <= 0xffff) return 3;
  return 4;
}

export function utf8ByteOffsetToUtf16Index(source: string, byteOffset: number): number {
  const target = Math.max(0, byteOffset);
  let bytes = 0;
  let utf16Index = 0;

  for (const scalar of source) {
    const scalarBytes = utf8ScalarWidth(scalar);
    if (bytes + scalarBytes > target) {
      return utf16Index;
    }
    bytes += scalarBytes;
    utf16Index += scalar.length;
  }

  return source.length;
}

export function sourceSelection(source: string, span: SourceSpan): SourceSelection {
  const start = utf8ByteOffsetToUtf16Index(source, span.start);
  const end = utf8ByteOffsetToUtf16Index(source, span.end);
  return { start, end: Math.max(start, end) };
}

export function sourceLocationLabel(source: string, span: SourceSpan): string {
  const start = utf8ByteOffsetToUtf16Index(source, span.start);
  const prefix = source.slice(0, start);
  const lines = prefix.split("\n");
  const line = lines.length;
  const column = Array.from(lines.at(-1) ?? "").length + 1;
  return `${span.file}:${line}:${column}`;
}
