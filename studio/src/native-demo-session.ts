import type { BridgeEnvelope, StudioDiagnostic } from "./protocol";

export type NativeDemoSessionState<Result> =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "running" }>
  | Readonly<{ kind: "ready"; result: Result }>
  | Readonly<{
      kind: "failed";
      diagnostics: readonly StudioDiagnostic[];
      message: string;
    }>;

type NativeDemoRunner<Result> = () => Promise<BridgeEnvelope<Result>>;
type NativeDemoObserver<Result> = (state: NativeDemoSessionState<Result>) => void;

/** Generation-guarded publication shared by closed, single-response native examples. */
export class NativeDemoSession<Result> {
  readonly #runNative: NativeDemoRunner<Result>;
  readonly #fallbackMessage: string;
  readonly #observer: NativeDemoObserver<Result>;
  #generation = 0;
  #state: NativeDemoSessionState<Result> = { kind: "idle" };

  constructor(
    runNative: NativeDemoRunner<Result>,
    fallbackMessage: string,
    observer: NativeDemoObserver<Result> = () => {},
  ) {
    this.#runNative = runNative;
    this.#fallbackMessage = fallbackMessage;
    this.#observer = observer;
  }

  get state(): NativeDemoSessionState<Result> {
    return this.#state;
  }

  clear(): void {
    this.#generation += 1;
    this.#transition({ kind: "idle" });
  }

  async run(): Promise<NativeDemoSessionState<Result>> {
    const generation = ++this.#generation;
    this.#transition({ kind: "running" });
    const response = await this.#runNative();
    if (generation !== this.#generation) return this.#state;
    if (response.result !== null) {
      this.#transition({ kind: "ready", result: response.result });
      return this.#state;
    }
    const message =
      response.diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ??
      this.#fallbackMessage;
    this.#transition({
      kind: "failed",
      diagnostics: response.diagnostics,
      message,
    });
    return this.#state;
  }

  #transition(state: NativeDemoSessionState<Result>): void {
    this.#state = state;
    this.#observer(state);
  }
}
