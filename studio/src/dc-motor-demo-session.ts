import type { StudioBridge } from "./bridge";
import type { DcMotorDemoResult } from "./dc-motor-demo-protocol";
import { BRIDGE_PROTOCOL, type StudioDiagnostic } from "./protocol";

export type DcMotorDemoSessionState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "running" }>
  | Readonly<{ kind: "ready"; result: DcMotorDemoResult }>
  | Readonly<{
      kind: "failed";
      diagnostics: readonly StudioDiagnostic[];
      message: string;
    }>;

export type DcMotorDemoSessionObserver = (state: DcMotorDemoSessionState) => void;

/** Generation-guarded publication of one closed native DC-drive payload. */
export class DcMotorDemoSession {
  readonly #bridge: Pick<StudioBridge, "runDcMotorDemo">;
  readonly #observer: DcMotorDemoSessionObserver;
  #generation = 0;
  #state: DcMotorDemoSessionState = { kind: "idle" };

  constructor(
    bridge: Pick<StudioBridge, "runDcMotorDemo">,
    observer: DcMotorDemoSessionObserver = () => {},
  ) {
    this.#bridge = bridge;
    this.#observer = observer;
  }

  get state(): DcMotorDemoSessionState {
    return this.#state;
  }

  clear(): void {
    this.#generation += 1;
    this.#transition({ kind: "idle" });
  }

  async run(): Promise<DcMotorDemoSessionState> {
    const generation = ++this.#generation;
    this.#transition({ kind: "running" });
    const response = await this.#bridge.runDcMotorDemo({ protocol: BRIDGE_PROTOCOL });
    if (generation !== this.#generation) return this.#state;
    if (response.result !== null) {
      this.#transition({ kind: "ready", result: response.result });
      return this.#state;
    }
    const message =
      response.diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ??
      "The native packaged DC-drive demo did not return an accepted result.";
    this.#transition({
      kind: "failed",
      diagnostics: response.diagnostics,
      message,
    });
    return this.#state;
  }

  #transition(state: DcMotorDemoSessionState): void {
    this.#state = state;
    this.#observer(state);
  }
}
