import { z } from "zod";
import { BRIDGE_PROTOCOL } from "./protocol";
import {
  artifactDigestSchema,
  unstructuredFieldContextSchema,
} from "./unstructured-field-protocol";

export const CYLINDER_DEMO_PROTOCOL = "eqiora.studio.cylinder-stokes-demo/v1" as const;
export const CYLINDER_DEMO_ID = "steady-flow-past-cylinder" as const;

const finiteVector2Schema = z.tuple([z.number().finite(), z.number().finite()]);

export const cylinderDemoRequestSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
  })
  .strict();

export type CylinderDemoRequest = z.infer<typeof cylinderDemoRequestSchema>;

export const cylinderDemoResultSchema = z
  .object({
    protocol: z.literal(CYLINDER_DEMO_PROTOCOL),
    exampleId: z.literal(CYLINDER_DEMO_ID),
    context: unstructuredFieldContextSchema,
    geometry: z
      .object({
        exactSourceDigest: artifactDigestSchema,
        realizedGeometryDigest: artifactDigestSchema,
        requestedMaxBoundaryErrorM: z.number().positive().finite(),
        boundaryEvaluationAllowanceM: z.number().positive().finite(),
        boundaryErrorBoundM: z.number().positive().finite(),
        circleSegments: z.literal(50),
      })
      .strict(),
    cylinderReaction: z
      .object({
        convention: z.literal("constraint-force-on-fluid"),
        forceOnFluidNM: finiteVector2Schema,
      })
      .strict(),
    fluxBalance: z
      .object({
        convention: z.literal("physical-parent-outward"),
        inletM2S: z.number().finite(),
        outletM2S: z.number().finite(),
        netM2S: z.number().finite(),
      })
      .strict(),
    momentumBalance: z
      .object({
        constrainedReactionNM: finiteVector2Schema,
        integratedBodyForceNM: finiteVector2Schema,
        integratedTractionNM: finiteVector2Schema,
        closureNM: finiteVector2Schema,
      })
      .strict(),
    solver: z
      .object({
        algorithm: z.literal("sparse-lu"),
        preconditioner: z.literal("identity"),
        reduction: z.literal("fast"),
        relativeTolerance: z.literal(1e-6),
        absoluteTolerance: z.literal(1e-13),
        completedIterations: z.number().int().nonnegative(),
        residualTarget: z.number().positive().finite(),
        trueResidualNorm: z.number().nonnegative().finite(),
        continuityResidualNorm: z.number().nonnegative().finite(),
      })
      .strict(),
  })
  .strict()
  .superRefine((result, refinement) => {
    if (
      result.geometry.boundaryEvaluationAllowanceM >= result.geometry.requestedMaxBoundaryErrorM ||
      result.geometry.boundaryErrorBoundM > result.geometry.requestedMaxBoundaryErrorM
    ) {
      refinement.addIssue({
        code: "custom",
        message: "Cylinder geometry evidence exceeds its accepted approximation request.",
        path: ["geometry", "boundaryErrorBoundM"],
      });
    }
    if (result.context.field.coherentSiUnit !== "kg·m^-1·s^-2") {
      refinement.addIssue({
        code: "custom",
        message: "Cylinder demo must publish the accepted coherent-SI pressure field.",
        path: ["context", "field", "coherentSiUnit"],
      });
    }
    if (result.fluxBalance.netM2S !== result.fluxBalance.inletM2S + result.fluxBalance.outletM2S) {
      refinement.addIssue({
        code: "custom",
        message: "Cylinder flux balance is not the exact sum of its signed boundary fluxes.",
        path: ["fluxBalance", "netM2S"],
      });
    }
    for (const component of [0, 1] as const) {
      const closure =
        result.momentumBalance.constrainedReactionNM[component] +
        result.momentumBalance.integratedBodyForceNM[component] +
        result.momentumBalance.integratedTractionNM[component];
      if (result.momentumBalance.closureNM[component] !== closure) {
        refinement.addIssue({
          code: "custom",
          message: "Cylinder momentum closure differs from its three retained terms.",
          path: ["momentumBalance", "closureNM", component],
        });
      }
    }
    if (result.solver.trueResidualNorm > result.solver.residualTarget) {
      refinement.addIssue({
        code: "custom",
        message: "Cylinder solve did not satisfy its independently reapplied residual target.",
        path: ["solver", "trueResidualNorm"],
      });
    }
  });

export type CylinderDemoResult = z.infer<typeof cylinderDemoResultSchema>;
