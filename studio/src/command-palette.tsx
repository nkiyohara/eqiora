import { useEffect, useMemo, useRef } from "react";
import { type CommandId, matchingCommands } from "./commands";

export type CommandAvailability = Readonly<
  Record<CommandId, Readonly<{ enabled: boolean; reason: string | null }>>
>;

interface CommandPaletteProps {
  readonly open: boolean;
  readonly query: string;
  readonly availability: CommandAvailability;
  readonly onClose: () => void;
  readonly onQuery: (query: string) => void;
  readonly onExecute: (command: CommandId) => void;
}

export function CommandPalette({
  open,
  query,
  availability,
  onClose,
  onQuery,
  onExecute,
}: CommandPaletteProps) {
  const dialog = useRef<HTMLDialogElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const commands = useMemo(() => matchingCommands(query), [query]);

  useEffect(() => {
    const element = dialog.current;
    if (element === null) return;
    if (open && !element.open) {
      element.showModal();
      window.requestAnimationFrame(() => input.current?.focus());
    } else if (!open && element.open) {
      element.close();
    }
  }, [open]);

  return (
    <dialog
      aria-labelledby="command-palette-heading"
      className="command-palette"
      ref={dialog}
      onCancel={(event) => {
        event.preventDefault();
        onClose();
      }}
    >
      <div className="command-palette__heading">
        <div>
          <span className="eyebrow">Spatial-memory-independent actions</span>
          <h2 id="command-palette-heading">Commands</h2>
        </div>
        <button
          aria-label="Close commands"
          className="command-palette__close"
          onClick={onClose}
          type="button"
        >
          Esc
        </button>
      </div>
      <label className="command-search">
        <span className="sr-only">Search commands</span>
        <input
          ref={input}
          autoComplete="off"
          onChange={(event) => onQuery(event.currentTarget.value)}
          onKeyDown={(event) => {
            if (event.key === "ArrowDown") {
              event.preventDefault();
              dialog.current
                ?.querySelector<HTMLButtonElement>(".command-item:not(:disabled)")
                ?.focus();
            }
          }}
          placeholder="Search model, run, or view actions"
          type="search"
          value={query}
        />
        <kbd>Ctrl/⌘ K</kbd>
      </label>
      <ul className="command-list">
        {commands.length === 0 ? (
          <li className="command-list__empty">No command matches this search.</li>
        ) : (
          commands.map((command) => {
            const state = availability[command.id];
            const reasonId = `command-reason-${command.id.replaceAll(".", "-")}`;
            return (
              <li key={command.id}>
                <button
                  aria-describedby={state.reason === null ? undefined : reasonId}
                  className="command-item"
                  disabled={!state.enabled}
                  onClick={() => {
                    onClose();
                    window.requestAnimationFrame(() => onExecute(command.id));
                  }}
                  type="button"
                >
                  <span className="command-item__group">{command.group}</span>
                  <span className="command-item__copy">
                    <strong>{command.label}</strong>
                    <small>{command.description}</small>
                    {state.reason === null ? null : <small id={reasonId}>{state.reason}</small>}
                  </span>
                  {command.shortcut === null ? null : <kbd>{command.shortcut}</kbd>}
                </button>
              </li>
            );
          })
        )}
      </ul>
    </dialog>
  );
}
