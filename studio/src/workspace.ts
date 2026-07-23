import { z } from "zod";
import type { NodeLayout } from "./state";

export const WORKSPACE_PROTOCOL = "eqiora.studio.workspace/v1" as const;
export const MAX_WORKSPACE_NODES = 100_000;
const MAX_WORKSPACE_BYTES = 16 * 1_024 * 1_024;
const MAX_COORDINATE = 10_000_000;

const pointSchema = z.object({
  x: z.number().finite().min(-MAX_COORDINATE).max(MAX_COORDINATE),
  y: z.number().finite().min(-MAX_COORDINATE).max(MAX_COORDINATE),
});

export const workspaceEnvelopeSchema = z.object({
  protocol: z.literal(WORKSPACE_PROTOCOL),
  modelDigest: z.string().min(16).max(128),
  layout: z
    .record(z.string().min(1).max(128), pointSchema)
    .refine((layout) => Object.keys(layout).length <= MAX_WORKSPACE_NODES, {
      message: `Workspace layout exceeds ${MAX_WORKSPACE_NODES.toLocaleString()} nodes.`,
    }),
});

export type WorkspaceEnvelope = z.infer<typeof workspaceEnvelopeSchema>;

export interface WorkspaceStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

export function workspaceStorageKey(digest: string): string {
  return `${WORKSPACE_PROTOCOL}:${digest}`;
}

export function decodeWorkspace(serialized: string, expectedDigest: string): NodeLayout | null {
  if (new TextEncoder().encode(serialized).length > MAX_WORKSPACE_BYTES) return null;
  try {
    const decoded = workspaceEnvelopeSchema.safeParse(JSON.parse(serialized));
    if (!decoded.success || decoded.data.modelDigest !== expectedDigest) return null;
    return decoded.data.layout;
  } catch {
    return null;
  }
}

export function encodeWorkspace(modelDigest: string, layout: NodeLayout): string {
  const orderedLayout = Object.fromEntries(
    Object.entries(layout).sort(([left], [right]) => left.localeCompare(right)),
  );
  return JSON.stringify(
    workspaceEnvelopeSchema.parse({
      protocol: WORKSPACE_PROTOCOL,
      modelDigest,
      layout: orderedLayout,
    }),
  );
}

export function readWorkspace(storage: WorkspaceStorage, digest: string): NodeLayout | null {
  try {
    const serialized = storage.getItem(workspaceStorageKey(digest));
    return serialized === null ? null : decodeWorkspace(serialized, digest);
  } catch {
    return null;
  }
}

export function writeWorkspace(
  storage: WorkspaceStorage,
  digest: string,
  layout: NodeLayout,
): boolean {
  try {
    storage.setItem(workspaceStorageKey(digest), encodeWorkspace(digest, layout));
    return true;
  } catch {
    return false;
  }
}
