import { COMMAND_REGISTRY, type CommandGroup, type CommandId } from "./application";
import { formatMessage } from "./messages";

export type { CommandId } from "./application";

export interface CommandDefinition {
  readonly id: CommandId;
  readonly group: "Model" | "Execution" | "View" | "Navigate";
  readonly label: string;
  readonly description: string;
  readonly shortcut: string | null;
}

function groupLabel(group: CommandGroup): CommandDefinition["group"] {
  return formatMessage(`command.group.${group}`);
}

export const COMMANDS: readonly CommandDefinition[] = COMMAND_REGISTRY.map((command) => ({
  id: command.id,
  group: groupLabel(command.group),
  label: formatMessage(command.label),
  description: formatMessage(command.description),
  shortcut: command.shortcut,
}));

export function matchingCommands(query: string): readonly CommandDefinition[] {
  const terms = query.trim().toLocaleLowerCase().split(/\s+/u).filter(Boolean);
  if (terms.length === 0) return COMMANDS;
  return COMMANDS.filter((command) => {
    const searchable =
      `${command.group} ${command.label} ${command.description}`.toLocaleLowerCase();
    return terms.every((term) => searchable.includes(term));
  });
}
