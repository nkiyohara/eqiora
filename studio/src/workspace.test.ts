import { describe, expect, it } from "vitest";
import {
  decodeWorkspace,
  encodeWorkspace,
  readWorkspace,
  WORKSPACE_PROTOCOL,
  workspaceStorageKey,
  writeWorkspace,
} from "./workspace";

const DIGEST = "sha256:0123456789abcdef";

describe("workspace-only persistence", () => {
  it("round-trips a deterministic schema independently of model bytes", () => {
    const encoded = encodeWorkspace(DIGEST, {
      "Relation:z": { x: 20, y: 30 },
      "Field:a": { x: 1, y: 2 },
    });
    expect(encoded).toBe(
      `{"protocol":"${WORKSPACE_PROTOCOL}","modelDigest":"${DIGEST}","layout":{"Field:a":{"x":1,"y":2},"Relation:z":{"x":20,"y":30}}}`,
    );
    expect(decodeWorkspace(encoded, DIGEST)).toEqual({
      "Field:a": { x: 1, y: 2 },
      "Relation:z": { x: 20, y: 30 },
    });
  });

  it("fails closed on another digest, version, non-finite data, or malformed JSON", () => {
    const valid = encodeWorkspace(DIGEST, { "Field:a": { x: 1, y: 2 } });
    expect(decodeWorkspace(valid, "sha256:fedcba9876543210")).toBeNull();
    expect(
      decodeWorkspace(valid.replace(WORKSPACE_PROTOCOL, "eqiora.studio.workspace/v2"), DIGEST),
    ).toBeNull();
    expect(decodeWorkspace('{"layout":', DIGEST)).toBeNull();
    expect(
      decodeWorkspace(
        JSON.stringify({
          protocol: WORKSPACE_PROTOCOL,
          modelDigest: DIGEST,
          layout: { "Field:a": { x: 20_000_000, y: 0 } },
        }),
        DIGEST,
      ),
    ).toBeNull();
  });

  it("contains storage failure without affecting the caller", () => {
    const storage = {
      getItem: () => {
        throw new Error("unavailable");
      },
      setItem: () => {
        throw new Error("quota");
      },
    };
    expect(readWorkspace(storage, DIGEST)).toBeNull();
    expect(writeWorkspace(storage, DIGEST, {})).toBe(false);
    expect(workspaceStorageKey(DIGEST)).toContain(DIGEST);
  });
});
