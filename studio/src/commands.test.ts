import { describe, expect, it } from "vitest";
import { COMMANDS, matchingCommands } from "./commands";

describe("command catalog", () => {
  it("keeps every command identity unique", () => {
    expect(new Set(COMMANDS.map((command) => command.id)).size).toBe(COMMANDS.length);
  });

  it("matches all query terms across group, label, and description", () => {
    expect(matchingCommands("three-package").map((command) => command.id)).toEqual([
      "example.dc-drive",
    ]);
    expect(matchingCommands("focus canonical").map((command) => command.id)).toEqual([
      "focus.source",
      "focus.relation",
    ]);
    expect(matchingCommands("   ")).toBe(COMMANDS);
  });
});
