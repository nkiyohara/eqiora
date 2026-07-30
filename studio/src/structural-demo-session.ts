import type { StudioBridge } from "./bridge";
import { BRIDGE_PROTOCOL, type StudioDiagnostic } from "./protocol";
import type { StructuralDemoResult } from "./structural-demo-protocol";

export type StructuralDemoSessionState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "running" }>
  | Readonly<{ kind: "ready"; result: StructuralDemoResult }>
  | Readonly<{
      kind: "failed";
      diagnostics: readonly StudioDiagnostic[];
      message: string;
    }>;

export type StructuralDemoSessionObserver = (state: StructuralDemoSessionState) => void;

/** Generation-guarded publication of one closed native structural payload. */
export class StructuralDemoSession {
  readonly #bridge: Pick<StudioBridge, "runStructuralDemo">;
  readonly #observer: StructuralDemoSessionObserver;
  #generation = 0;
  #state: StructuralDemoSessionState = { kind: "idle" };

  constructor(
    bridge: Pick<StudioBridge, "runStructuralDemo">,
    observer: StructuralDemoSessionObserver = () => {},
  ) {
    this.#bridge = bridge;
    this.#observer = observer;
  }

  get state(): StructuralDemoSessionState {
    return this.#state;
  }

  clear(): void {
    this.#generation += 1;
    this.#transition({ kind: "idle" });
  }

  async run(): Promise<StructuralDemoSessionState> {
    const generation = ++this.#generation;
    this.#transition({ kind: "running" });
    const response = await this.#bridge.runStructuralDemo({ protocol: BRIDGE_PROTOCOL });
    if (generation !== this.#generation) return this.#state;
    if (response.result !== null) {
      this.#transition({ kind: "ready", result: response.result });
      return this.#state;
    }
    const message =
      response.diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ??
      "The native structural demonstration did not return an accepted result.";
    this.#transition({
      kind: "failed",
      diagnostics: response.diagnostics,
      message,
    });
    return this.#state;
  }

  #transition(state: StructuralDemoSessionState): void {
    this.#state = state;
    this.#observer(state);
  }
}
