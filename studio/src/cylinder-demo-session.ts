import type { StudioBridge } from "./bridge";
import type { CylinderDemoResult } from "./cylinder-demo-protocol";
import { BRIDGE_PROTOCOL, type StudioDiagnostic } from "./protocol";
import type { UnstructuredFieldDataBridge } from "./unstructured-field-bridge";
import {
  UnstructuredFieldDataSession,
  type UnstructuredFieldSessionState,
} from "./unstructured-field-session";

type ReadyFieldState = Extract<UnstructuredFieldSessionState, { kind: "ready" }>;
type LoadingFieldState = Extract<UnstructuredFieldSessionState, { kind: "opening" | "streaming" }>;

export type CylinderDemoSessionState =
  | Readonly<{ kind: "idle" }>
  | Readonly<{ kind: "solving" }>
  | Readonly<{
      kind: "loading-field";
      result: CylinderDemoResult;
      field: LoadingFieldState;
    }>
  | Readonly<{
      kind: "ready";
      result: CylinderDemoResult;
      field: ReadyFieldState;
    }>
  | Readonly<{
      kind: "failed";
      result: CylinderDemoResult | null;
      diagnostics: readonly StudioDiagnostic[];
      message: string;
    }>;

export type CylinderDemoSessionObserver = (state: CylinderDemoSessionState) => void;

/** Fail-closed command-to-field composition for the immutable cylinder demo. */
export class CylinderDemoSession {
  readonly #command: Pick<StudioBridge, "runCylinderDemo">;
  readonly #field: UnstructuredFieldDataSession;
  readonly #observer: CylinderDemoSessionObserver;
  #generation = 0;
  #result: CylinderDemoResult | null = null;
  #state: CylinderDemoSessionState = { kind: "idle" };

  constructor(
    command: Pick<StudioBridge, "runCylinderDemo">,
    fieldBridge: UnstructuredFieldDataBridge,
    observer: CylinderDemoSessionObserver = () => {},
  ) {
    this.#command = command;
    this.#field = new UnstructuredFieldDataSession(fieldBridge, (state) =>
      this.#acceptFieldState(state),
    );
    this.#observer = observer;
  }

  get state(): CylinderDemoSessionState {
    return this.#state;
  }

  clear(): void {
    this.#generation += 1;
    this.#result = null;
    this.#field.clear();
    this.#transition({ kind: "idle" });
  }

  async run(): Promise<CylinderDemoSessionState> {
    const generation = ++this.#generation;
    this.#result = null;
    this.#field.clear();
    this.#transition({ kind: "solving" });
    const response = await this.#command.runCylinderDemo({
      protocol: BRIDGE_PROTOCOL,
    });
    if (generation !== this.#generation) return this.#state;
    if (response.result === null) {
      const message =
        response.diagnostics.find((diagnostic) => diagnostic.severity === "error")?.message ??
        "The native cylinder demo did not return an accepted result.";
      this.#transition({
        kind: "failed",
        result: null,
        diagnostics: response.diagnostics,
        message,
      });
      return this.#state;
    }
    this.#result = response.result;
    await this.#field.load(response.result.context);
    return this.#state;
  }

  #acceptFieldState(field: UnstructuredFieldSessionState): void {
    const result = this.#result;
    if (result === null) return;
    switch (field.kind) {
      case "idle":
        return;
      case "opening":
      case "streaming":
        this.#transition({ kind: "loading-field", result, field });
        return;
      case "ready":
        this.#transition({ kind: "ready", result, field });
        return;
      case "failed":
        this.#transition({
          kind: "failed",
          result,
          diagnostics: [],
          message: field.failure.cause?.message ?? field.failure.message,
        });
        return;
    }
  }

  #transition(state: CylinderDemoSessionState): void {
    this.#state = state;
    this.#observer(state);
  }
}
