import { z } from "zod";
import { artifactDigestSchema, BRIDGE_PROTOCOL } from "./protocol";

export const DC_MOTOR_DEMO_PROTOCOL = "eqiora.studio.packaged-dc-drive-demo/v2" as const;
export const DC_MOTOR_DEMO_ID = "packaged-dc-motor-control" as const;
export const DC_MOTOR_SCIENTIFIC_CASE = "hybrid.packaged-dc-motor-controller" as const;

const ACCEPTED_STEPS = 100;
const SAMPLE_PERIOD_STEPS = 10;

export const dcMotorDemoRequestSchema = z
  .object({
    protocol: z.literal(BRIDGE_PROTOCOL),
  })
  .strict();

export type DcMotorDemoRequest = z.infer<typeof dcMotorDemoRequestSchema>;

const trajectorySampleSchema = z
  .object({
    step: z.number().int().min(0).max(ACCEPTED_STEPS),
    timeS: z.number().finite().min(0).max(0.1),
    currentA: z.number().finite(),
    angularSpeedPerS: z.number().finite(),
    heldVoltageV: z.number().finite(),
  })
  .strict();

const commitSchema = z
  .object({
    step: z.number().int().min(0).max(ACCEPTED_STEPS),
    timeS: z.number().finite().min(0).max(0.1),
    heldVoltageV: z.number().finite(),
  })
  .strict();

const packageNodeSchema = z
  .object({
    name: z.enum([
      "Eqiora.Electrical.Basic",
      "Eqiora.Electromechanical.DcDrive",
      "org.example.dc_motor_control",
    ]),
    version: z.literal("0.1.0"),
    semanticDigest: artifactDigestSchema,
    sourceDigest: artifactDigestSchema,
  })
  .strict();

const packageLabelSchema = z.enum([
  "Eqiora.Electrical.Basic@0.1.0",
  "Eqiora.Electromechanical.DcDrive@0.1.0",
  "org.example.dc_motor_control@0.1.0",
]);

export const dcMotorDemoResultSchema = z
  .object({
    protocol: z.literal(DC_MOTOR_DEMO_PROTOCOL),
    exampleId: z.literal(DC_MOTOR_DEMO_ID),
    trajectory: z
      .object({
        samples: z.array(trajectorySampleSchema).length(ACCEPTED_STEPS + 1),
        commits: z.array(commitSchema).length(ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS + 1),
        units: z
          .object({
            current: z.literal("A"),
            angularSpeed: z.literal("s^-1"),
            heldVoltage: z.literal("V"),
            time: z.literal("s"),
          })
          .strict(),
      })
      .strict(),
    packageGraph: z
      .object({
        root: z.literal("org.example.dc_motor_control@0.1.0"),
        resolutionDigest: artifactDigestSchema,
        nodes: z.array(packageNodeSchema).length(3),
        edges: z
          .array(
            z
              .object({
                declaring: packageLabelSchema,
                alias: z.enum(["drive", "electrical"]),
                target: packageLabelSchema,
              })
              .strict(),
          )
          .length(3),
      })
      .strict(),
    execution: z
      .object({
        method: z.literal("backward-euler"),
        scalarType: z.literal("f64"),
        placement: z.literal("one-host-one-worker"),
        endTimeS: z.literal(0.1),
        maximumStepS: z.literal(0.001),
        samplePeriodS: z.literal(0.01),
        acceptedSteps: z.literal(ACCEPTED_STEPS),
        holdIntervals: z.literal(ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS),
        controllerCommits: z.literal(ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS + 1),
      })
      .strict(),
    lineage: z
      .object({
        modelDigest: artifactDigestSchema,
        compilationDigest: artifactDigestSchema,
        runDigest: artifactDigestSchema,
        runBindingDigest: artifactDigestSchema,
        semanticRevision: z.number().int().nonnegative(),
      })
      .strict(),
    evidence: z
      .object({
        caseId: z.literal(DC_MOTOR_SCIENTIFIC_CASE),
        status: z.literal("historical-a3-only"),
        physicalPortSamplesPresented: z.literal(false),
      })
      .strict(),
  })
  .strict()
  .superRefine((result, refinement) => {
    const samples = result.trajectory.samples;
    for (let step = 0; step <= ACCEPTED_STEPS; step += 1) {
      const sample = samples[step];
      if (sample === undefined) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive trajectory omitted an integer-step boundary.",
          path: ["trajectory", "samples", step],
        });
        continue;
      }
      if (sample.step !== step || sample.timeS !== step * 0.001) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive samples must retain exact integer-step provenance.",
          path: ["trajectory", "samples", step],
        });
      }
    }

    const commits = result.trajectory.commits;
    for (let ordinal = 0; ordinal < commits.length; ordinal += 1) {
      const step = ordinal * SAMPLE_PERIOD_STEPS;
      const commit = commits[ordinal];
      const sample = samples[step];
      if (commit === undefined || sample === undefined) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive commit ledger omitted a required boundary.",
          path: ["trajectory", "commits", ordinal],
        });
        continue;
      }
      if (
        commit.step !== step ||
        commit.timeS !== sample.timeS ||
        commit.heldVoltageV !== sample.heldVoltageV
      ) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive commit ledger differs from its trajectory boundary.",
          path: ["trajectory", "commits", ordinal],
        });
      }
    }

    for (let interval = 0; interval < ACCEPTED_STEPS / SAMPLE_PERIOD_STEPS; interval += 1) {
      const start = interval * SAMPLE_PERIOD_STEPS;
      const first = samples[start];
      if (first === undefined) continue;
      const held = first.heldVoltageV;
      if (
        samples
          .slice(start, start + SAMPLE_PERIOD_STEPS)
          .some((sample) => sample.heldVoltageV !== held)
      ) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive voltage is not zero-order held for one complete sample interval.",
          path: ["trajectory", "samples", start, "heldVoltageV"],
        });
      }
    }

    const nodeNames = new Set(
      result.packageGraph.nodes.map((node) => `${node.name}@${node.version}`),
    );
    if (nodeNames.size !== 3 || !nodeNames.has(result.packageGraph.root)) {
      refinement.addIssue({
        code: "custom",
        message: "DC-drive package graph does not contain its exact root closure.",
        path: ["packageGraph", "nodes"],
      });
    }
    for (const [edge, value] of result.packageGraph.edges.entries()) {
      if (!nodeNames.has(value.declaring) || !nodeNames.has(value.target)) {
        refinement.addIssue({
          code: "custom",
          message: "DC-drive package edge names a package outside the exact closure.",
          path: ["packageGraph", "edges", edge],
        });
      }
    }
    const expectedEdges = new Set([
      "Eqiora.Electromechanical.DcDrive@0.1.0|electrical|Eqiora.Electrical.Basic@0.1.0",
      "org.example.dc_motor_control@0.1.0|drive|Eqiora.Electromechanical.DcDrive@0.1.0",
      "org.example.dc_motor_control@0.1.0|electrical|Eqiora.Electrical.Basic@0.1.0",
    ]);
    const actualEdges = new Set(
      result.packageGraph.edges.map((edge) => `${edge.declaring}|${edge.alias}|${edge.target}`),
    );
    if (
      actualEdges.size !== expectedEdges.size ||
      [...expectedEdges].some((edge) => !actualEdges.has(edge))
    ) {
      refinement.addIssue({
        code: "custom",
        message: "DC-drive package edges differ from the exact pinned dependency closure.",
        path: ["packageGraph", "edges"],
      });
    }
  });

export type DcMotorDemoResult = z.infer<typeof dcMotorDemoResultSchema>;
