import { z } from "zod";
import { BRIDGE_PROTOCOL } from "./protocol";
import { artifactDigestSchema } from "./unstructured-field-protocol";

export const FSI_DEMO_PROTOCOL = "eqiora.studio.fixed-reference-fsi-demo/v1" as const;
export const FSI_DEMO_ID = "fixed-reference-monolithic-fsi-step" as const;
export const FSI_STEP_CASE = "fsi.fixed-reference-monolithic-step-2d" as const;
export const FSI_TRAJECTORY_CASE = "artifacts.fixed-reference-fsi-spatial-trajectory" as const;
export const FSI_VERTEX_COUNT = 9;
export const FSI_CELL_COUNT = 8;
export const FSI_INTERFACE_FACET_COUNT = 2;

export const fsiDemoRequestSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
  })
  .strict();

export type FsiDemoRequest = z.infer<typeof fsiDemoRequestSchema>;

const vector2Schema = z.tuple([z.number().finite(), z.number().finite()]);
const vertexIndexSchema = z
  .number()
  .int()
  .min(0)
  .max(FSI_VERTEX_COUNT - 1);

const vertexSchema = z
  .object({
    index: vertexIndexSchema,
    coordinatesM: vector2Schema,
  })
  .strict();

const cellSchema = z
  .object({
    index: z
      .number()
      .int()
      .min(0)
      .max(FSI_CELL_COUNT - 1),
    vertices: z.tuple([vertexIndexSchema, vertexIndexSchema, vertexIndexSchema]),
    region: z.enum(["fluid", "solid"]),
  })
  .strict();

const interfaceFacetSchema = z
  .object({
    index: z.number().int().nonnegative(),
    vertices: z.tuple([vertexIndexSchema, vertexIndexSchema]),
  })
  .strict();

const vectorFieldSchema = (unit: "m/s" | "m", length: number) =>
  z
    .object({
      unit: z.literal(unit),
      values: z.array(vector2Schema).length(length),
    })
    .strict();

const physicsAcceptanceSchema = z
  .object({
    numericalResidualNorm: z.number().finite().nonnegative(),
    continuityResidualNorm: z.number().finite().nonnegative(),
    kinematicResidualNorm: z.number().finite().nonnegative(),
    interfaceVelocityJumpNorm: z.literal(0),
    interfaceActionImbalanceNPerM: z.number().finite().nonnegative(),
    absoluteEnergyDefectJPerM: z.number().finite().nonnegative(),
  })
  .strict();

const stepSchema = z
  .object({
    step: z.union([z.literal(1), z.literal(2)]),
    timeS: z.union([z.literal(0.05), z.literal(0.1)]),
    velocity: vectorFieldSchema("m/s", FSI_VERTEX_COUNT),
    fluidBubbleVelocity: vectorFieldSchema("m/s", 4),
    pressure: z
      .object({
        unit: z.literal("kg m^-1 s^-2"),
        supportVertices: z.array(vertexIndexSchema).length(6),
        values: z.array(z.number().finite()).length(6),
      })
      .strict(),
    displacement: vectorFieldSchema("m", FSI_VERTEX_COUNT),
    interfaceActions: z
      .array(
        z
          .object({
            vertex: vertexIndexSchema,
            unit: z.literal("N/m"),
            fluid: vector2Schema,
            solid: vector2Schema,
            imbalance: vector2Schema,
          })
          .strict(),
      )
      .length(1),
    energy: z
      .object({
        unit: z.literal("J/m"),
        previousKinetic: z.number().finite().nonnegative(),
        nextKinetic: z.number().finite().nonnegative(),
        previousElastic: z.number().finite().nonnegative(),
        nextElastic: z.number().finite().nonnegative(),
        kineticIncrement: z.number().finite().nonnegative(),
        elasticIncrement: z.number().finite().nonnegative(),
        viscousDissipation: z.number().finite().nonnegative(),
        defect: z.number().finite(),
      })
      .strict(),
    physicsAcceptance: physicsAcceptanceSchema,
    solverStopping: z
      .object({
        convergenceReason: z.enum(["initial-residual-satisfied", "residual-tolerance-satisfied"]),
        completedIterations: z.number().int().nonnegative().max(20_000),
        trueResidualNorm: z.number().finite().nonnegative(),
        residualTarget: z.number().finite().positive(),
      })
      .strict(),
    assembly: z
      .object({
        packetCount: z.number().int().positive(),
        targetCount: z.number().int().positive(),
      })
      .strict(),
  })
  .strict();

const EXPECTED_VERTICES = [
  [0, 0],
  [1, 0],
  [0, 0.5],
  [1, 0.5],
  [0, 1],
  [1, 1],
  [2, 0],
  [2, 0.5],
  [2, 1],
] as const;

const EXPECTED_CELLS = [
  [0, 1, 3],
  [0, 3, 2],
  [2, 3, 5],
  [2, 5, 4],
  [1, 6, 7],
  [1, 7, 3],
  [3, 7, 8],
  [3, 8, 5],
] as const;

export const fsiDemoResultSchema = z
  .object({
    protocol: z.literal(FSI_DEMO_PROTOCOL),
    exampleId: z.literal(FSI_DEMO_ID),
    mesh: z
      .object({
        vertices: z.array(vertexSchema).length(FSI_VERTEX_COUNT),
        cells: z.array(cellSchema).length(FSI_CELL_COUNT),
        interfaceFacets: z.array(interfaceFacetSchema).length(FSI_INTERFACE_FACET_COUNT),
      })
      .strict(),
    steps: z.array(stepSchema).length(2),
    execution: z
      .object({
        method: z.literal("fixed-reference-monolithic-fsi"),
        fluidSpace: z.literal("continuous-mini-velocity-p1-pressure"),
        solidSpace: z.literal("continuous-p1-velocity-displacement"),
        timeMethod: z.literal("backward-euler"),
        timeStepS: z.literal(0.05),
        lengthScaleM: z.literal(2),
        velocityScaleMPerS: z.literal(0.5),
        pressureScalePa: z.literal(4),
        scalarType: z.literal("f64"),
        placement: z.literal("one-host-one-worker"),
        solver: z.literal("minimum-residual"),
        preconditioner: z.literal("identity"),
        reduction: z.literal("reproducible"),
        relativeTolerance: z.literal(1e-11),
        absoluteTolerance: z.literal(1e-13),
      })
      .strict(),
    lineage: z
      .object({
        modelDigest: artifactDigestSchema,
        geometryDigest: artifactDigestSchema,
        correspondenceDigest: artifactDigestSchema,
        meshDigest: artifactDigestSchema,
        realizationDigest: artifactDigestSchema,
        runDigest: artifactDigestSchema,
        stateDigests: z.array(artifactDigestSchema).length(2),
        trajectoryDigest: artifactDigestSchema,
        semanticRevision: z.number().int().nonnegative(),
        realizationRevision: z.literal(1),
        runOutputArtifacts: z.literal(1),
      })
      .strict(),
    evidence: z.tuple([
      z
        .object({
          caseId: z.literal(FSI_STEP_CASE),
          status: z.literal("verified"),
        })
        .strict(),
      z
        .object({
          caseId: z.literal(FSI_TRAJECTORY_CASE),
          status: z.literal("verified"),
        })
        .strict(),
    ]),
  })
  .strict()
  .superRefine((result, refinement) => {
    result.mesh.vertices.forEach((vertex, index) => {
      const expected = EXPECTED_VERTICES[index];
      if (
        vertex.index !== index ||
        expected === undefined ||
        vertex.coordinatesM[0] !== expected[0] ||
        vertex.coordinatesM[1] !== expected[1]
      ) {
        refinement.addIssue({
          code: "custom",
          message: "FSI vertices must retain exact physical coordinates and order.",
          path: ["mesh", "vertices", index],
        });
      }
    });
    result.mesh.cells.forEach((cell, index) => {
      const expected = EXPECTED_CELLS[index];
      const region = index < 4 ? "fluid" : "solid";
      if (
        cell.index !== index ||
        cell.region !== region ||
        expected === undefined ||
        cell.vertices.some((vertex, local) => vertex !== expected[local])
      ) {
        refinement.addIssue({
          code: "custom",
          message: "FSI cells must retain exact ordered two-body connectivity.",
          path: ["mesh", "cells", index],
        });
      }
    });
    const interfaceEdges = new Set(
      result.mesh.interfaceFacets.map(({ vertices }) => [...vertices].sort().join(":")),
    );
    if (interfaceEdges.size !== 2 || !interfaceEdges.has("1:3") || !interfaceEdges.has("3:5")) {
      refinement.addIssue({
        code: "custom",
        message: "FSI interface must retain the complete conforming x=1 side.",
        path: ["mesh", "interfaceFacets"],
      });
    }
    result.steps.forEach((step, index) => {
      if (step.step !== index + 1 || step.timeS !== (index + 1) * 0.05) {
        refinement.addIssue({
          code: "custom",
          message: "FSI steps must be consecutive accepted coordinates.",
          path: ["steps", index],
        });
      }
      if (
        step.pressure.supportVertices.some((vertex, ordinal) => vertex !== ordinal) ||
        [0, 2, 4].some(
          (vertex) =>
            step.displacement.values[vertex]?.[0] !== 0 ||
            step.displacement.values[vertex]?.[1] !== 0,
        )
      ) {
        refinement.addIssue({
          code: "custom",
          message: "FSI Field values escaped their exact physical supports.",
          path: ["steps", index],
        });
      }
      if (step.interfaceActions[0]?.vertex !== 3) {
        refinement.addIssue({
          code: "custom",
          message: "FSI interface action must belong to the one free midpoint.",
          path: ["steps", index, "interfaceActions"],
        });
      }
      const acceptance = step.physicsAcceptance;
      if (
        step.solverStopping.trueResidualNorm > step.solverStopping.residualTarget ||
        acceptance.numericalResidualNorm >= 1e-9 ||
        acceptance.continuityResidualNorm >= 1e-9 ||
        acceptance.kinematicResidualNorm >= 1e-14 ||
        acceptance.interfaceActionImbalanceNPerM >= 1e-9 ||
        acceptance.absoluteEnergyDefectJPerM >= 1e-9
      ) {
        refinement.addIssue({
          code: "custom",
          message: "FSI payload contains an unaccepted solver or physics result.",
          path: ["steps", index, "physicsAcceptance"],
        });
      }
    });
    if (
      result.steps[0]?.displacement.values.every((value, vertex) => {
        const next = result.steps[1]?.displacement.values[vertex];
        return next?.[0] === value[0] && next[1] === value[1];
      })
    ) {
      refinement.addIssue({
        code: "custom",
        message: "FSI second step duplicated the first accepted state.",
        path: ["steps", 1],
      });
    }
  });

export type FsiDemoResult = z.infer<typeof fsiDemoResultSchema>;
