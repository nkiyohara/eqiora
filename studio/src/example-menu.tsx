import type { CommandAvailability } from "./command-palette";
import type { CommandId } from "./commands";
import "./example-menu.css";

type ExampleStatus = "idle" | "running" | "ready" | "failed";

export interface ExampleMenuProps {
  readonly availability: CommandAvailability;
  readonly cylinderRunning: boolean;
  readonly dcMotorStatus: ExampleStatus;
  readonly fsiStatus: ExampleStatus;
  readonly structuralStatus: ExampleStatus;
  readonly onExecute: (command: CommandId) => void;
}

const EXAMPLES = [
  {
    command: "example.fsi",
    glyph: "⇄",
    idleLabel: "Fluid–structure interaction",
    runningLabel: "Solving coupled steps…",
    detail: "2 bodies · 1 shared trace · 2 accepted steps",
  },
  {
    command: "example.structural",
    glyph: "⌗",
    idleLabel: "Elastic panel",
    runningLabel: "Solving structure…",
    detail: "2D Q1 displacement · reaction evidence",
  },
  {
    command: "example.dc-drive",
    glyph: "⌁",
    idleLabel: "Sampled DC drive",
    runningLabel: "Running DC drive…",
    detail: "3 packages · 100 steps · held control",
  },
  {
    command: "example.cylinder",
    glyph: "◯",
    idleLabel: "Exact-cylinder Stokes",
    runningLabel: "Solving cylinder…",
    detail: "Exact source · bounded pressure field",
  },
] as const satisfies readonly {
  command: Extract<CommandId, `example.${string}`>;
  glyph: string;
  idleLabel: string;
  runningLabel: string;
  detail: string;
}[];

export function ExampleMenu({
  availability,
  cylinderRunning,
  dcMotorStatus,
  fsiStatus,
  structuralStatus,
  onExecute,
}: ExampleMenuProps) {
  const running = {
    "example.cylinder": cylinderRunning,
    "example.dc-drive": dcMotorStatus === "running",
    "example.fsi": fsiStatus === "running",
    "example.structural": structuralStatus === "running",
  } as const;
  return (
    <details className="example-menu">
      <summary className="secondary-action">Examples</summary>
      <div className="example-menu__panel">
        <span className="eyebrow">Immutable native examples</span>
        {EXAMPLES.map((example) => (
          <button
            disabled={!availability[example.command].enabled}
            key={example.command}
            onClick={(event) => {
              event.currentTarget.closest("details")?.removeAttribute("open");
              onExecute(example.command);
            }}
            title={availability[example.command].reason ?? undefined}
            type="button"
          >
            <span aria-hidden="true">{example.glyph}</span>
            <strong>{running[example.command] ? example.runningLabel : example.idleLabel}</strong>
            <small>{example.detail}</small>
          </button>
        ))}
      </div>
    </details>
  );
}
