import { describe, expect, it } from "vitest";
import { COMMANDS, matchingCommands } from "./commands";

describe("command catalog", () => {
  it("keeps every command identity unique", () => {
    expect(new Set(COMMANDS.map((command) => command.id)).size).toBe(COMMANDS.length);
  });

  it("matches all query terms across group, label, and description", () => {
    expect(matchingCommands("native plan").map((command) => command.id)).toEqual(["run.execute"]);
    expect(matchingCommands("cancel safe").map((command) => command.id)).toEqual(["run.cancel"]);
    expect(matchingCommands("focus canonical").map((command) => command.id)).toEqual([
      "focus.source",
      "focus.relation",
    ]);
    expect(matchingCommands("   ")).toBe(COMMANDS);
  });
});
