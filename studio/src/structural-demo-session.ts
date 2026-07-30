import type { StudioBridge } from "./bridge";
import { NativeDemoSession, type NativeDemoSessionState } from "./native-demo-session";
import { BRIDGE_PROTOCOL } from "./protocol";
import type { StructuralDemoResult } from "./structural-demo-protocol";

export type StructuralDemoSessionState = NativeDemoSessionState<StructuralDemoResult>;

export type StructuralDemoSessionObserver = (state: StructuralDemoSessionState) => void;

/** Closed native structural response with stale-generation suppression. */
export class StructuralDemoSession extends NativeDemoSession<StructuralDemoResult> {
  constructor(
    bridge: Pick<StudioBridge, "runStructuralDemo">,
    observer: StructuralDemoSessionObserver = () => {},
  ) {
    super(
      () => bridge.runStructuralDemo({ protocol: BRIDGE_PROTOCOL }),
      "The native structural demonstration did not return an accepted result.",
      observer,
    );
  }
}
