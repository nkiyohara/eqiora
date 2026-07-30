import type { StudioBridge } from "./bridge";
import type { DcMotorDemoResult } from "./dc-motor-demo-protocol";
import { NativeDemoSession, type NativeDemoSessionState } from "./native-demo-session";
import { BRIDGE_PROTOCOL } from "./protocol";

export type DcMotorDemoSessionState = NativeDemoSessionState<DcMotorDemoResult>;

export type DcMotorDemoSessionObserver = (state: DcMotorDemoSessionState) => void;

/** Closed native DC-drive response with stale-generation suppression. */
export class DcMotorDemoSession extends NativeDemoSession<DcMotorDemoResult> {
  constructor(
    bridge: Pick<StudioBridge, "runDcMotorDemo">,
    observer: DcMotorDemoSessionObserver = () => {},
  ) {
    super(
      () => bridge.runDcMotorDemo({ protocol: BRIDGE_PROTOCOL }),
      "The native packaged DC-drive demo did not return an accepted result.",
      observer,
    );
  }
}
