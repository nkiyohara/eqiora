import { invoke } from "@tauri-apps/api/core";
import type { ZodType } from "zod";
import { checkedRequest, protocolFailure } from "./bridge-contract";
import {
  type DcMotorDemoRequest,
  type DcMotorDemoResult,
  dcMotorDemoRequestSchema,
  dcMotorDemoResultSchema,
} from "./dc-motor-demo-protocol";
import { type BridgeEnvelope, bridgeEnvelopeSchema } from "./protocol";

type DemoContract<Request, Result> = Readonly<{
  label: string;
  command: string;
  request: ZodType<Request>;
  result: ZodType<Result>;
  previewRefusal: string;
}>;

const DC_MOTOR = {
  label: "Packaged DC-drive demo",
  command: "run_dc_motor_demo",
  request: dcMotorDemoRequestSchema,
  result: dcMotorDemoResultSchema,
  previewRefusal:
    "The packaged DC-drive execution is available only in native Studio; browser preview does not fabricate scientific results.",
} satisfies DemoContract<DcMotorDemoRequest, DcMotorDemoResult>;

async function runNativeDemo<Request, Result>(
  contract: DemoContract<Request, Result>,
  request: Request,
): Promise<BridgeEnvelope<Result>> {
  const checked = checkedRequest(contract.request, request, contract.label);
  if (!checked.ok) return checked.failure;
  try {
    const response: unknown = await invoke(contract.command, { request: checked.value });
    const decoded = bridgeEnvelopeSchema(contract.result).safeParse(response);
    return decoded.success
      ? decoded.data
      : protocolFailure(`Native bridge returned an invalid ${contract.command} response.`);
  } catch (error: unknown) {
    const detail = error instanceof Error ? error.message : String(error);
    return protocolFailure(`Native bridge call ${contract.command} failed: ${detail}`);
  }
}

function rejectPreviewDemo<Request, Result>(
  contract: DemoContract<Request, Result>,
  request: Request,
): BridgeEnvelope<Result> {
  const checked = checkedRequest(contract.request, request, contract.label);
  return checked.ok ? protocolFailure(contract.previewRefusal) : checked.failure;
}

export const nativeDemoBridge = {
  runDcMotor: (request: DcMotorDemoRequest) => runNativeDemo(DC_MOTOR, request),
} as const;

export const previewDemoBridge = {
  runDcMotor: (request: DcMotorDemoRequest) => rejectPreviewDemo(DC_MOTOR, request),
} as const;
