import { z } from "zod";

/**
 * Closed sub-protocol for the authored-CAD operation-history workspace.
 * Independently versioned; it never widens `eqiora.studio.cad/v1`.
 *
 * Every scientific value on these schemas is an exact observation replayed by
 * the native Rust owner. The client validates structure, frozen constants,
 * and field-for-field echoes only — it never computes a digest, clearance,
 * bound, area, volume, tolerance, or lineage. Canonical graph bytes and face
 * handles are opaque bounded lowercase-hex tokens the client can only echo
 * back.
 */
export const CAD_AUTHORED_PROTOCOL = "eqiora.studio.cad-authored/v1" as const;

/** Owner decoder admits at most 4096 canonical graph bytes. */
export const CAD_AUTHORED_MAX_GRAPH_BYTES = 4_096;
/** Owner decoder admits at most 512 canonical handle bytes. */
export const CAD_AUTHORED_MAX_HANDLE_BYTES = 512;

/** Admitted face provenance, in the owner's canonical inventory order. */
export const CAD_AUTHORED_V1_FACE_KEYS = [
  "start-cap",
  "end-cap",
  "profile-x-lower",
  "profile-x-upper",
  "profile-y-lower",
  "profile-y-upper",
] as const;
export const CAD_AUTHORED_V2_FACE_KEYS = [...CAD_AUTHORED_V1_FACE_KEYS, "cut-wall"] as const;

const RECTANGLE_PROFILE = "eqiora.cad.analytic-rectangle-extrusion-v1" as const;
const CIRCULAR_CUT_PROFILE = "eqiora.cad.analytic-circular-through-cut-v1" as const;

const digestSchema = z.string().regex(/^[0-9a-f]{64}$/);
const graphHexSchema = z
  .string()
  .regex(/^(?:[0-9a-f]{2})+$/)
  .max(2 * CAD_AUTHORED_MAX_GRAPH_BYTES);
const handleHexSchema = z
  .string()
  .regex(/^(?:[0-9a-f]{2})+$/)
  .max(2 * CAD_AUTHORED_MAX_HANDLE_BYTES);
// Finiteness, positivity, and interval order are pure request ergonomics that
// never narrow the native owner's valid set; every numeric admission decision
// beyond them belongs to the owner alone.
const finiteScalarSchema = z.number().finite();
const positiveScalarSchema = finiteScalarSchema.refine((value) => value > 0, {
  message: "Authored-CAD scalar must be strictly positive.",
});
const intervalSchema = z
  .tuple([z.number().finite(), z.number().finite()])
  .refine(([lower, upper]) => upper > lower, "Authored-CAD interval must have positive extent.");
const vec2Schema = z.tuple([z.number().finite(), z.number().finite()]);
const vec3Schema = z.tuple([z.number().finite(), z.number().finite(), z.number().finite()]);

export const cadAuthoredFaceKeySchema = z.enum(CAD_AUTHORED_V2_FACE_KEYS);
export type CadAuthoredFaceKey = z.infer<typeof cadAuthoredFaceKeySchema>;

/**
 * The one closed build request: rectangle-extrusion scalars plus either no
 * cut or one cut object. There is no operations array, node enum, provider
 * selector, or client-authored canonical form.
 */
export const cadAuthoredBuildRequestSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_PROTOCOL),
    sketch: z
      .object({
        xBoundsM: intervalSchema,
        yBoundsM: intervalSchema,
        planeZM: finiteScalarSchema,
      })
      .strict(),
    extrusionDepthM: positiveScalarSchema,
    requestedModelingToleranceM: positiveScalarSchema,
    cut: z
      .object({
        centerM: z.tuple([finiteScalarSchema, finiteScalarSchema]),
        radiusM: positiveScalarSchema,
        requestedBooleanToleranceM: positiveScalarSchema,
      })
      .strict()
      .nullable(),
  })
  .strict();

export type GeometryBuildReceiptRequest = z.infer<typeof cadAuthoredBuildRequestSchema>;

const historyOperationSchema = z.discriminatedUnion("kind", [
  z
    .object({
      kind: z.literal("sketch-plane"),
      id: z.literal("sketch-plane"),
      plane: z.literal("xy"),
      zM: z.number().finite(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("rectangle-profile"),
      id: z.literal("rectangle-profile"),
      sketchPlane: z.literal("sketch-plane"),
      constraint: z.literal("closed-by-construction"),
      xBoundsM: intervalSchema,
      yBoundsM: intervalSchema,
    })
    .strict(),
  z
    .object({
      kind: z.literal("closed-face"),
      id: z.literal("profile-face"),
      profile: z.literal("rectangle-profile"),
      regionCount: z.literal(1),
    })
    .strict(),
  z
    .object({
      kind: z.literal("positive-z-extrusion"),
      id: z.literal("positive-z-extrusion"),
      face: z.literal("profile-face"),
      depthM: z.number().finite().positive(),
      repair: z.literal("none"),
    })
    .strict(),
  z
    .object({
      kind: z.literal("cut-sketch-plane"),
      id: z.literal("cut-sketch-plane"),
      face: z.literal("end-cap"),
    })
    .strict(),
  z
    .object({
      kind: z.literal("circle-profile"),
      id: z.literal("circle-profile"),
      sketchPlane: z.literal("cut-sketch-plane"),
      constraint: z.literal("closed-by-construction"),
      centerM: vec2Schema,
      radiusM: z.number().finite().positive(),
    })
    .strict(),
  z
    .object({
      kind: z.literal("closed-cut-face"),
      id: z.literal("cut-profile-face"),
      profile: z.literal("circle-profile"),
      regionCount: z.literal(1),
    })
    .strict(),
  z
    .object({
      kind: z.literal("circular-through-cut"),
      id: z.literal("circular-through-cut"),
      target: z.literal("positive-z-extrusion"),
      toolFace: z.literal("cut-profile-face"),
      requestedBooleanToleranceM: z.number().finite().positive(),
      repair: z.literal("none"),
    })
    .strict(),
]);

export type CadAuthoredOperation = z.infer<typeof historyOperationSchema>;

/** The complete accepted v2 dependency chain, in frozen wire order. */
const HISTORY_ORDER = [
  "sketch-plane",
  "rectangle-profile",
  "closed-face",
  "positive-z-extrusion",
  "cut-sketch-plane",
  "circle-profile",
  "closed-cut-face",
  "circular-through-cut",
] as const;

const cadAuthoredFaceSchema = z
  .object({
    provenanceKey: cadAuthoredFaceKeySchema,
    handleHex: handleHexSchema,
    areaM2: z.number().finite().positive(),
    boundaryLoopCount: z.union([z.literal(1), z.literal(2)]),
    centroidM: vec3Schema.nullable(),
    outwardUnitNormal: vec3Schema.nullable(),
    verticesM: z.array(vec3Schema).length(4).nullable(),
  })
  .strict();

export type CadAuthoredFace = z.infer<typeof cadAuthoredFaceSchema>;

const lineageHandleSchema = z
  .object({
    provenanceKey: cadAuthoredFaceKeySchema,
    handleHex: handleHexSchema,
  })
  .strict();

export type CadAuthoredLineageHandle = z.infer<typeof lineageHandleSchema>;

const lineageListSchema = z.array(lineageHandleSchema).max(CAD_AUTHORED_V2_FACE_KEYS.length);

const cadAuthoredLineageSchema = z
  .object({
    retainedUnchanged: lineageListSchema,
    retainedModified: lineageListSchema,
    created: lineageListSchema,
    deleted: lineageListSchema,
    split: lineageListSchema,
    merged: lineageListSchema,
  })
  .strict();

export type CadAuthoredLineage = z.infer<typeof cadAuthoredLineageSchema>;

const cadAuthoredBuildReceiptSchema = z
  .object({
    graphDigest: digestSchema,
    providerProfile: z.enum([RECTANGLE_PROFILE, CIRCULAR_CUT_PROFILE]),
    requestedModelingToleranceM: z.number().finite().positive(),
    requestedBooleanToleranceM: z.number().finite().positive().nullable(),
    effectiveBooleanToleranceM: z.number().finite().positive().nullable(),
    maximumPositionDiscrepancyM: z.number().finite().nonnegative(),
    maximumAreaDiscrepancyM2: z.number().finite().nonnegative(),
    maximumVolumeDiscrepancyM3: z.number().finite().nonnegative(),
    repair: z.literal("none"),
    lineage: cadAuthoredLineageSchema,
  })
  .strict();

export type GeometryBuildReceiptReceipt = z.infer<typeof cadAuthoredBuildReceiptSchema>;

const cadAuthoredObservationsSchema = z
  .object({
    boundsM: z.tuple([intervalSchema, intervalSchema, intervalSchema]),
    outerVerticesM: z.array(vec3Schema).length(8),
    vertexCount: z.literal(8).nullable(),
    edgeCount: z.literal(12).nullable(),
    faceCount: z.union([z.literal(6), z.literal(7)]),
    closedShellCount: z.literal(1),
    bodyCount: z.literal(1),
    genus: z.union([z.literal(0), z.literal(1)]),
    volumeM3: z.number().finite().positive(),
    surfaceAreaM2: z.number().finite().positive(),
  })
  .strict();

export type CadAuthoredObservations = z.infer<typeof cadAuthoredObservationsSchema>;

function lineageEntries(lineage: CadAuthoredLineage): CadAuthoredLineageHandle[] {
  return [
    ...lineage.retainedUnchanged,
    ...lineage.retainedModified,
    ...lineage.created,
    ...lineage.deleted,
    ...lineage.split,
    ...lineage.merged,
  ];
}

/**
 * Complete projection of one accepted authored history.
 *
 * `graphDigest` is the authored *graph* identity. It is deliberately never
 * named a Geometry identity: replaying the same scalars with only a
 * different requested tolerance keeps every observation while changing it.
 */
export const cadAuthoredProjectionSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_PROTOCOL),
    graphDigest: digestSchema,
    canonicalGraphHex: graphHexSchema,
    canonicalByteCount: z.number().int().positive().max(CAD_AUTHORED_MAX_GRAPH_BYTES),
    history: z.array(historyOperationSchema).min(4).max(8),
    tolerances: z
      .object({
        requestedModelingToleranceM: z.number().finite().positive(),
        requestedBooleanToleranceM: z.number().finite().positive().nullable(),
        repair: z.literal("none"),
      })
      .strict(),
    observations: cadAuthoredObservationsSchema,
    faces: z.array(cadAuthoredFaceSchema).min(6).max(7),
    build: cadAuthoredBuildReceiptSchema,
  })
  .strict()
  .superRefine((projection, context) => {
    if (2 * projection.canonicalByteCount !== projection.canonicalGraphHex.length) {
      context.addIssue({
        code: "custom",
        message: "Opaque canonical bytes and their byte count disagree.",
        path: ["canonicalByteCount"],
      });
    }
    if (projection.build.graphDigest !== projection.graphDigest) {
      context.addIssue({
        code: "custom",
        message: "Build receipt realizes a different authored graph identity.",
        path: ["build", "graphDigest"],
      });
    }

    if (projection.history.length !== 4 && projection.history.length !== 8) {
      context.addIssue({
        code: "custom",
        message: "Authored history must be the complete 4-step or 8-step frozen chain.",
        path: ["history"],
      });
      return;
    }
    for (const [index, expected] of HISTORY_ORDER.slice(0, projection.history.length).entries()) {
      if (projection.history[index]?.kind !== expected) {
        context.addIssue({
          code: "custom",
          message: "Authored history operations are out of canonical order.",
          path: ["history", index, "kind"],
        });
        return;
      }
    }

    const hasCut = projection.history.length === 8;
    const expectedKeys = hasCut ? CAD_AUTHORED_V2_FACE_KEYS : CAD_AUTHORED_V1_FACE_KEYS;
    const coherentVariant =
      projection.observations.faceCount === expectedKeys.length &&
      projection.observations.genus === (hasCut ? 1 : 0) &&
      (projection.observations.vertexCount === null) === hasCut &&
      (projection.observations.edgeCount === null) === hasCut &&
      (projection.tolerances.requestedBooleanToleranceM !== null) === hasCut &&
      (projection.build.requestedBooleanToleranceM !== null) === hasCut &&
      projection.build.providerProfile === (hasCut ? CIRCULAR_CUT_PROFILE : RECTANGLE_PROFILE);
    if (!coherentVariant) {
      context.addIssue({
        code: "custom",
        message: "Cut-bearing history and its observations, tolerances, and receipt disagree.",
        path: ["observations"],
      });
    }

    if (
      projection.build.requestedModelingToleranceM !==
      projection.tolerances.requestedModelingToleranceM
    ) {
      context.addIssue({
        code: "custom",
        message: "Build receipt echoes a different requested modeling tolerance.",
        path: ["build", "requestedModelingToleranceM"],
      });
    }
    if (
      projection.build.requestedBooleanToleranceM !==
      projection.tolerances.requestedBooleanToleranceM
    ) {
      context.addIssue({
        code: "custom",
        message: "Build receipt echoes a different requested Boolean tolerance.",
        path: ["build", "requestedBooleanToleranceM"],
      });
    }
    // Both accepted profiles are no-substitution: a cut-free history has no
    // Boolean stage, and the cut applies exactly the requested tolerance.
    const expectedEffectiveBooleanToleranceM = hasCut
      ? projection.tolerances.requestedBooleanToleranceM
      : null;
    if (projection.build.effectiveBooleanToleranceM !== expectedEffectiveBooleanToleranceM) {
      context.addIssue({
        code: "custom",
        message: "Effective Boolean tolerance departs from the accepted no-substitution profile.",
        path: ["build", "effectiveBooleanToleranceM"],
      });
    }
    const lastOperation = projection.history[projection.history.length - 1];
    if (
      hasCut &&
      lastOperation?.kind === "circular-through-cut" &&
      lastOperation.requestedBooleanToleranceM !== projection.tolerances.requestedBooleanToleranceM
    ) {
      context.addIssue({
        code: "custom",
        message: "History cut operation requests a different Boolean tolerance than projected.",
        path: ["history", projection.history.length - 1, "requestedBooleanToleranceM"],
      });
    }
    if (
      projection.faces.map((face) => face.provenanceKey).join() !== expectedKeys.join() ||
      new Set(projection.faces.map((face) => face.handleHex)).size !== projection.faces.length
    ) {
      context.addIssue({
        code: "custom",
        message: "Face inventory differs from the admitted canonical provenance order.",
        path: ["faces"],
      });
    }

    const handleByKey = new Map(
      projection.faces.map((face) => [face.provenanceKey, face.handleHex]),
    );
    for (const entry of lineageEntries(projection.build.lineage)) {
      if (handleByKey.get(entry.provenanceKey) !== entry.handleHex) {
        context.addIssue({
          code: "custom",
          message: "Lineage handle is not the exact graph-bound handle of its face.",
          path: ["build", "lineage"],
        });
        return;
      }
    }
  });

export type CadAuthoredProjection = z.infer<typeof cadAuthoredProjectionSchema>;

/** The only accepted face-selection replay request shape. */
export const cadAuthoredSelectionRequestSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_PROTOCOL),
    graphDigest: digestSchema,
    canonicalGraphHex: graphHexSchema,
    handleHex: handleHexSchema,
  })
  .strict();

export type CadAuthoredSelectionRequest = z.infer<typeof cadAuthoredSelectionRequestSchema>;

export const cadAuthoredSelectionResultSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_PROTOCOL),
    graphDigest: digestSchema,
    provenanceKey: cadAuthoredFaceKeySchema,
    handleHex: handleHexSchema,
    areaM2: z.number().finite().positive(),
    boundaryLoopCount: z.union([z.literal(1), z.literal(2)]),
    centroidM: vec3Schema.nullable(),
    outwardUnitNormal: vec3Schema.nullable(),
  })
  .strict();

export type CadAuthoredSelectionResult = z.infer<typeof cadAuthoredSelectionResultSchema>;

/**
 * Closed Python-export sub-protocol, independently versioned beside the
 * authored-CAD projection protocol. The client validates the bounded native
 * rendering and displays it; it never generates, reformats, hashes, repairs,
 * or semantically interprets Python. Field names, mutations, and dispositions
 * are frozen by the hostile corpus at
 * `verify/interfaces/studio-python-cad-round-trip/models/hostile.json`.
 */
export const CAD_AUTHORED_EXPORT_PROTOCOL = "eqiora.studio.cad-authored-python-export/v1" as const;
/** The one suggested filename; there is no filename template or selector. */
export const CAD_AUTHORED_EXPORT_FILE_NAME = "eqiora_authored_cad.py" as const;
/** The generated projection for the two admitted histories stays bounded. */
export const CAD_AUTHORED_EXPORT_MAX_SOURCE_BYTES = 4_096;

/**
 * The one closed export request: opaque canonical graph bytes plus the exact
 * digest they must replay to. A request carries neither a filesystem path nor
 * Python source; the native side replays, renders, and (for saves) writes
 * only through its own dialog-chosen path.
 */
export const cadAuthoredExportRequestSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_EXPORT_PROTOCOL),
    canonicalGraphHex: graphHexSchema,
    graphDigest: digestSchema,
  })
  .strict();

export type CadAuthoredExportRequest = z.infer<typeof cadAuthoredExportRequestSchema>;

/**
 * Bounded structural rejection of a source projection, per the corpus: size,
 * NUL, CR, final newline, non-public imports, and canonical-blob replay are
 * closed string checks — never Python parsing or regeneration.
 */
function boundedSourceRejection(source: string): string | null {
  if (new TextEncoder().encode(source).length > CAD_AUTHORED_EXPORT_MAX_SOURCE_BYTES) {
    return "Generated Python source exceeds its bounded size.";
  }
  if (source.startsWith("\ufeff")) return "Generated Python source contains a UTF-8 BOM.";
  if (source.includes("\u0000")) return "Generated Python source contains a NUL byte.";
  if (source.includes("\r")) return "Generated Python source contains a CR line ending.";
  if (!source.endsWith("\n") || source.endsWith("\n\n")) {
    return "Generated Python source must end with exactly one LF newline.";
  }
  for (const line of source.split("\n")) {
    if ((line.startsWith("import ") || line.startsWith("from ")) && line !== "import eqiora") {
      return "Generated Python source may import only the public eqiora package.";
    }
  }
  if (source.includes("decode_canonical")) {
    return "Generated Python source may not replay a canonical wire blob.";
  }
  return null;
}

const sourceUtf8Schema = z.string().superRefine((source, context) => {
  const rejection = boundedSourceRejection(source);
  if (rejection !== null) {
    context.addIssue({ code: "custom", message: rejection });
  }
});

/** The exact closed render response: nothing more than the corpus fields. */
export const cadAuthoredExportRenderSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_EXPORT_PROTOCOL),
    graphDigest: digestSchema,
    suggestedFileName: z.literal(CAD_AUTHORED_EXPORT_FILE_NAME),
    sourceUtf8: sourceUtf8Schema,
  })
  .strict();

export type CadAuthoredExportRender = z.infer<typeof cadAuthoredExportRenderSchema>;

/**
 * The exact closed save response. Cancellation is a normal explicit outcome;
 * a write error arrives as a diagnostic, never as a third status.
 */
export const cadAuthoredExportSaveSchema = z
  .object({
    protocol: z.literal(CAD_AUTHORED_EXPORT_PROTOCOL),
    graphDigest: digestSchema,
    status: z.enum(["saved", "cancelled"]),
  })
  .strict();

export type CadAuthoredExportSave = z.infer<typeof cadAuthoredExportSaveSchema>;
