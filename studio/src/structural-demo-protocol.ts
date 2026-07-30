import { z } from "zod";
import { BRIDGE_PROTOCOL } from "./protocol";
import { artifactDigestSchema } from "./unstructured-field-protocol";

export const STRUCTURAL_DEMO_PROTOCOL = "eqiora.studio.mixed-boundary-elasticity-demo/v1" as const;
export const STRUCTURAL_DEMO_ID = "mixed-boundary-linear-elasticity" as const;
export const STRUCTURAL_SCIENTIFIC_CASE = "solid.mixed-boundary-elasticity-2d" as const;

export const STRUCTURAL_CELLS_PER_AXIS = 16;
export const STRUCTURAL_VERTEX_COUNT = (STRUCTURAL_CELLS_PER_AXIS + 1) ** 2;
export const STRUCTURAL_CELL_COUNT = STRUCTURAL_CELLS_PER_AXIS ** 2;

export const structuralDemoRequestSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
  })
  .strict();

export type StructuralDemoRequest = z.infer<typeof structuralDemoRequestSchema>;

const vector2Schema = z.tuple([z.number().finite(), z.number().finite()]);

const vertexSchema = z
  .object({
    index: z
      .number()
      .int()
      .min(0)
      .max(STRUCTURAL_VERTEX_COUNT - 1),
    coordinatesM: vector2Schema,
  })
  .strict();

const cellSchema = z
  .object({
    index: z
      .number()
      .int()
      .min(0)
      .max(STRUCTURAL_CELL_COUNT - 1),
    vertices: z.tuple([
      z
        .number()
        .int()
        .min(0)
        .max(STRUCTURAL_VERTEX_COUNT - 1),
      z
        .number()
        .int()
        .min(0)
        .max(STRUCTURAL_VERTEX_COUNT - 1),
      z
        .number()
        .int()
        .min(0)
        .max(STRUCTURAL_VERTEX_COUNT - 1),
      z
        .number()
        .int()
        .min(0)
        .max(STRUCTURAL_VERTEX_COUNT - 1),
    ]),
  })
  .strict();

export const structuralDemoResultSchema = z
  .object({
    protocol: z.literal(STRUCTURAL_DEMO_PROTOCOL),
    exampleId: z.literal(STRUCTURAL_DEMO_ID),
    mesh: z
      .object({
        spatialDimension: z.literal(2),
        cellsPerAxis: z.literal(STRUCTURAL_CELLS_PER_AXIS),
        vertices: z.array(vertexSchema).length(STRUCTURAL_VERTEX_COUNT),
        cells: z.array(cellSchema).length(STRUCTURAL_CELL_COUNT),
      })
      .strict(),
    displacement: z
      .object({
        unit: z.literal("m"),
        valuesM: z.array(vector2Schema).length(STRUCTURAL_VERTEX_COUNT),
      })
      .strict(),
    balance: z
      .object({
        unit: z.literal("N"),
        constrainedReactionN: vector2Schema,
        integratedBodyForceN: vector2Schema,
      })
      .strict(),
    execution: z
      .object({
        method: z.literal("continuous-galerkin"),
        mesh: z.literal("generated-uniform-cartesian"),
        space: z.literal("continuous-q1-two-component"),
        quadrature: z.literal("gauss-legendre-2-per-axis"),
        scalarType: z.literal("f64"),
        placement: z.literal("one-host-one-worker"),
        solver: z.literal("conjugate-gradient"),
        preconditioner: z.literal("identity"),
        reduction: z.literal("reproducible"),
        convergenceReason: z.enum(["initial-residual-satisfied", "residual-tolerance-satisfied"]),
        relativeTolerance: z.literal(1e-12),
        absoluteTolerance: z.literal(1e-14),
        completedIterations: z.number().int().nonnegative().max(10_000),
        trueResidualNorm: z.number().finite().nonnegative(),
        residualTarget: z.number().finite().positive(),
        assemblyPackets: z.number().int().positive(),
        assemblyTargets: z.number().int().positive(),
      })
      .strict(),
    lineage: z
      .object({
        modelDigest: artifactDigestSchema,
        realizationDigest: artifactDigestSchema,
        runDigest: artifactDigestSchema,
        semanticRevision: z.number().int().nonnegative(),
        realizationRevision: z.literal(1),
        outputArtifacts: z.literal(0),
      })
      .strict(),
    evidence: z
      .object({
        caseId: z.literal(STRUCTURAL_SCIENTIFIC_CASE),
        status: z.literal("verified"),
      })
      .strict(),
  })
  .strict()
  .superRefine((result, refinement) => {
    for (let index = 0; index < result.mesh.vertices.length; index += 1) {
      const vertex = result.mesh.vertices[index];
      if (vertex?.index !== index) {
        refinement.addIssue({
          code: "custom",
          message: "Structural vertices must retain exact mesh order.",
          path: ["mesh", "vertices", index, "index"],
        });
      }
    }
    for (let index = 0; index < result.mesh.cells.length; index += 1) {
      const cell = result.mesh.cells[index];
      if (cell?.index !== index || new Set(cell?.vertices).size !== 4) {
        refinement.addIssue({
          code: "custom",
          message: "Structural cells must retain ordered, nondegenerate Q1 connectivity.",
          path: ["mesh", "cells", index],
        });
      }
    }
    if (result.execution.trueResidualNorm > result.execution.residualTarget) {
      refinement.addIssue({
        code: "custom",
        message: "Structural payload contains an unaccepted true residual.",
        path: ["execution", "trueResidualNorm"],
      });
    }
  });

export type StructuralDemoResult = z.infer<typeof structuralDemoResultSchema>;
