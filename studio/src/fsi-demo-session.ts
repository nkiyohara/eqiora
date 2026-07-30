import type { StudioBridge } from "./bridge";
import type { FsiDemoResult } from "./fsi-demo-protocol";
import { NativeDemoSession, type NativeDemoSessionState } from "./native-demo-session";
import { BRIDGE_PROTOCOL } from "./protocol";

export type FsiDemoSessionState = NativeDemoSessionState<FsiDemoResult>;
export type FsiDemoSessionObserver = (state: FsiDemoSessionState) => void;

/** Closed native FSI response with stale-generation suppression. */
export class FsiDemoSession extends NativeDemoSession<FsiDemoResult> {
  constructor(
    bridge: Pick<StudioBridge, "runFsiDemo">,
    observer: FsiDemoSessionObserver = () => {},
  ) {
    super(
      () => bridge.runFsiDemo({ protocol: BRIDGE_PROTOCOL }),
      "The native fixed-reference FSI demonstration did not return an accepted result.",
      observer,
    );
  }
}
