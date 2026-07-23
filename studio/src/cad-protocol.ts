import { z } from "zod";

/** Closed sub-protocol for the first bounded CAD workspace. */
export const CAD_VIEW_PROTOCOL = "eqiora.studio.cad/v1" as const;
export const CAD_V1_VERTEX_COUNT = 8;
export const CAD_V1_TRIANGLE_COUNT = 12;
export const CAD_V1_SEMANTIC_ENTITY_COUNT = 7;
export const CAD_MAX_MESH_ENTITY_COUNT = 250_000;

const digestSchema = z.string().regex(/^[0-9a-f]{64}$/);
const entityIdSchema = z.string().min(1).max(128);
const finiteIntervalSchema = z
  .tuple([z.number().finite(), z.number().finite()])
  .refine(([lower, upper]) => upper > lower, "CAD interval must have positive extent.");
const boxBoundsSchema = z.tuple([finiteIntervalSchema, finiteIntervalSchema, finiteIntervalSchema]);

export const cadSemanticEntitySchema = z
  .object({
    domainId: entityIdSchema,
    name: z.string().min(1).max(512).nullable(),
    kind: z.enum(["body", "boundary"]),
    parentDomainId: entityIdSchema.nullable(),
    axis: z.number().int().min(0).max(2).nullable(),
    side: z.enum(["lower", "upper"]).nullable(),
    meshEntityCount: z.number().int().positive().max(CAD_MAX_MESH_ENTITY_COUNT),
    relationIds: z.array(entityIdSchema).max(100_000),
    portIds: z.array(entityIdSchema).max(100_000),
  })
  .strict()
  .superRefine((entity, context) => {
    const coherentBody =
      entity.kind === "body" &&
      entity.parentDomainId === null &&
      entity.axis === null &&
      entity.side === null;
    const coherentBoundary =
      entity.kind === "boundary" &&
      entity.parentDomainId !== null &&
      entity.axis !== null &&
      entity.side !== null;
    if (!coherentBody && !coherentBoundary) {
      context.addIssue({
        code: "custom",
        message: "CAD entity kind and semantic boundary role are incoherent.",
        path: ["kind"],
      });
    }
  });

export type CadSemanticEntity = z.infer<typeof cadSemanticEntitySchema>;

const cadObservationSchema = z
  .object({
    solidCount: z.literal(1),
    closedShellCount: z.literal(1),
    planarFaceCount: z.literal(6),
  })
  .strict();

export const cadProjectionSchema = z
  .object({
    protocol: z.literal(CAD_VIEW_PROTOCOL),
    planKey: digestSchema,
    modelDigest: digestSchema,
    geometryDigest: digestSchema,
    meshDigest: digestSchema,
    design: z
      .object({
        sourceUnit: z.literal("millimetre"),
        importedStockBoundsM: boxBoundsSchema,
        sketch: z
          .object({
            xBoundsM: finiteIntervalSchema,
            yBoundsM: finiteIntervalSchema,
            planeZM: z.number().finite(),
            remainingDegreesOfFreedom: z.literal(0),
          })
          .strict(),
        extrusion: z
          .object({
            direction: z.literal("positive-z"),
            depthM: z.number().finite().positive(),
          })
          .strict(),
        boolean: z.literal("intersection"),
        resultBoundsM: boxBoundsSchema,
      })
      .strict(),
    build: z
      .object({
        adapter: z.string().min(1).max(128),
        adapterVersion: z.string().min(1).max(64),
        kernel: z.string().min(1).max(128),
        kernelVersion: z.string().min(1).max(64),
        repair: z.literal("none"),
        importedStock: cadObservationSchema,
        extrudedTool: cadObservationSchema,
        intersection: cadObservationSchema,
      })
      .strict(),
    verticesM: z
      .array(z.tuple([z.number().finite(), z.number().finite(), z.number().finite()]))
      .length(CAD_V1_VERTEX_COUNT),
    triangles: z
      .array(
        z
          .object({
            domainId: entityIdSchema,
            vertexIndices: z.tuple([
              z
                .number()
                .int()
                .nonnegative()
                .max(CAD_V1_VERTEX_COUNT - 1),
              z
                .number()
                .int()
                .nonnegative()
                .max(CAD_V1_VERTEX_COUNT - 1),
              z
                .number()
                .int()
                .nonnegative()
                .max(CAD_V1_VERTEX_COUNT - 1),
            ]),
          })
          .strict(),
      )
      .length(CAD_V1_TRIANGLE_COUNT),
    entities: z.array(cadSemanticEntitySchema).length(CAD_V1_SEMANTIC_ENTITY_COUNT),
  })
  .strict()
  .superRefine((projection, context) => {
    const body = projection.entities.filter((entity) => entity.kind === "body");
    const boundaries = projection.entities.filter((entity) => entity.kind === "boundary");
    if (body.length !== 1 || boundaries.length !== 6) {
      context.addIssue({
        code: "custom",
        message: "CAD v1 requires exactly one body and its six Cartesian boundaries.",
        path: ["entities"],
      });
      return;
    }

    const bodyId = body[0]?.domainId;
    const domains = new Set<string>();
    const boundaryDomains = new Set<string>();
    const roles = new Set<string>();
    for (const [index, entity] of projection.entities.entries()) {
      if (domains.has(entity.domainId)) {
        context.addIssue({
          code: "custom",
          message: "CAD projection contains a duplicate Semantic Domain.",
          path: ["entities", index, "domainId"],
        });
      }
      domains.add(entity.domainId);
      if (entity.kind === "boundary") {
        boundaryDomains.add(entity.domainId);
        if (entity.parentDomainId !== bodyId) {
          context.addIssue({
            code: "custom",
            message: "Every CAD boundary must name the one projected body as parent.",
            path: ["entities", index, "parentDomainId"],
          });
        }
        const role = `${entity.axis}:${entity.side}`;
        if (roles.has(role)) {
          context.addIssue({
            code: "custom",
            message: "CAD projection contains a duplicate Cartesian boundary role.",
            path: ["entities", index, "axis"],
          });
        }
        roles.add(role);
      }
    }

    const triangleCounts = new Map<string, number>();
    for (const [triangleIndex, triangle] of projection.triangles.entries()) {
      if (!boundaryDomains.has(triangle.domainId)) {
        context.addIssue({
          code: "custom",
          message: "Render triangles must name an exact boundary Domain.",
          path: ["triangles", triangleIndex, "domainId"],
        });
      }
      if (new Set(triangle.vertexIndices).size !== 3) {
        context.addIssue({
          code: "custom",
          message: "Render triangle must contain three distinct vertices.",
          path: ["triangles", triangleIndex, "vertexIndices"],
        });
      }
      triangleCounts.set(triangle.domainId, (triangleCounts.get(triangle.domainId) ?? 0) + 1);
    }
    for (const boundary of boundaries) {
      if (triangleCounts.get(boundary.domainId) !== 2) {
        context.addIssue({
          code: "custom",
          message: "Every Cartesian boundary must have exactly two render triangles.",
          path: ["triangles"],
        });
      }
    }

    const result = projection.design.resultBoundsM;
    const sketch = projection.design.sketch;
    if (
      result[0][0] !== sketch.xBoundsM[0] ||
      result[0][1] !== sketch.xBoundsM[1] ||
      result[1][0] !== sketch.yBoundsM[0] ||
      result[1][1] !== sketch.yBoundsM[1] ||
      result[2][0] !== sketch.planeZM ||
      result[2][1] !== sketch.planeZM + projection.design.extrusion.depthM
    ) {
      context.addIssue({
        code: "custom",
        message: "CAD result bounds differ from the fully constrained extrusion.",
        path: ["design", "resultBoundsM"],
      });
    }
    for (const axis of [0, 1, 2] as const) {
      if (
        projection.design.importedStockBoundsM[axis][0] > result[axis][0] ||
        projection.design.importedStockBoundsM[axis][1] < result[axis][1]
      ) {
        context.addIssue({
          code: "custom",
          message: "CAD boolean result escapes its imported stock.",
          path: ["design", "resultBoundsM", axis],
        });
      }
    }
  });

export type CadProjection = z.infer<typeof cadProjectionSchema>;

export const cadProjectionRequestSchema = z
  .object({
    protocol: z.literal(CAD_VIEW_PROTOCOL),
    modelDigest: digestSchema,
  })
  .strict();

export type CadProjectionRequest = z.infer<typeof cadProjectionRequestSchema>;

/** The only accepted semantic-selection request shape. */
export const cadSelectionRequestSchema = z
  .object({
    protocol: z.literal(CAD_VIEW_PROTOCOL),
    modelDigest: digestSchema,
    planKey: digestSchema,
    geometryDigest: digestSchema,
    domainId: entityIdSchema,
  })
  .strict();

export type CadSelectionRequest = z.infer<typeof cadSelectionRequestSchema>;

export const cadSelectionResultSchema = z
  .object({
    protocol: z.literal(CAD_VIEW_PROTOCOL),
    modelDigest: digestSchema,
    planKey: digestSchema,
    geometryDigest: digestSchema,
    domainId: entityIdSchema,
    entity: cadSemanticEntitySchema,
  })
  .strict()
  .refine((selection) => selection.domainId === selection.entity.domainId, {
    message: "Accepted CAD selection and projected entity identify different Domains.",
    path: ["entity", "domainId"],
  });

export type CadSelectionResult = z.infer<typeof cadSelectionResultSchema>;
