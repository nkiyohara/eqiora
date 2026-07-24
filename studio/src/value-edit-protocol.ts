import { z } from "zod";
import { BRIDGE_PROTOCOL, documentProjectionSchema } from "./protocol";

const valueEditControlFields = {
  protocol: z.literal(BRIDGE_PROTOCOL),
  digest: z.string().min(16).max(128),
  targetId: z.string().min(1).max(128),
  value: z.number().finite(),
} as const;

export const valueEditPreviewRequestSchema = z.object(valueEditControlFields);
export type ValueEditPreviewRequest = z.infer<typeof valueEditPreviewRequestSchema>;

export const valueEditCommitRequestSchema = z.object({
  ...valueEditControlFields,
  planKey: z.string().min(1).max(256),
});
export type ValueEditCommitRequest = z.infer<typeof valueEditCommitRequestSchema>;

export const quantitySchema = z.object({
  value: z.number().finite(),
  dimension: z.string().max(128),
});

export const valueEditPlanSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    key: z.string().min(1).max(256),
    baseDigest: z.string().min(16).max(128),
    baseRevision: z.number().int().nonnegative(),
    targetId: z.string().min(1).max(128),
    before: quantitySchema,
    after: quantitySchema,
    transactionDigest: z.string().min(64).max(128),
  })
  .refine((plan) => plan.before.dimension === plan.after.dimension, {
    message: "Value edit cannot change physical dimension.",
    path: ["after", "dimension"],
  })
  .refine((plan) => plan.before.value !== plan.after.value, {
    message: "Value edit must change canonical content.",
    path: ["after", "value"],
  });
export type ValueEditPlan = z.infer<typeof valueEditPlanSchema>;

export const valueEditEvidenceSchema = z.object({
  plan: valueEditPlanSchema,
  resultDigest: z.string().min(16).max(128),
  resultRevision: z.number().int().nonnegative(),
});
export type ValueEditEvidence = z.infer<typeof valueEditEvidenceSchema>;

export const valueEditResultSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
    document: documentProjectionSchema,
    evidence: valueEditEvidenceSchema,
  })
  .refine(
    (result) =>
      result.document.digest === result.evidence.resultDigest &&
      result.document.revision === result.evidence.resultRevision,
    {
      message: "Edited document and transaction evidence lineage differ.",
      path: ["evidence"],
    },
  );
export type ValueEditResult = z.infer<typeof valueEditResultSchema>;
