import { CYLINDER_EVIDENCE_FOCUS_ID } from "./application";
import type { CylinderDemoResult } from "./cylinder-demo-protocol";
import type { UnstructuredFieldSessionState } from "./unstructured-field-session";
import { UnstructuredFieldWorkspace } from "./unstructured-field-workspace";
import "./cylinder-demo-workspace.css";

type ReadyFieldState = Extract<UnstructuredFieldSessionState, { kind: "ready" }>;

export interface CylinderDemoWorkspaceProps {
  readonly result: CylinderDemoResult;
  readonly field: ReadyFieldState;
  readonly selectedVertex: number;
  readonly onSelect: (vertex: number) => void;
}

export function CylinderDemoWorkspace({
  result,
  field,
  selectedVertex,
  onSelect,
}: CylinderDemoWorkspaceProps) {
  return (
    <div className="cylinder-demo-workspace">
      <section
        className="cylinder-demo-evidence"
        aria-labelledby="cylinder-demo-heading"
        id={CYLINDER_EVIDENCE_FOCUS_ID}
        tabIndex={-1}
      >
        <header>
          <div>
            <span className="eyebrow">Verified composition · immutable public example</span>
            <h1 id="cylinder-demo-heading">Steady flow past an exact circular cylinder</h1>
          </div>
          <div className="cylinder-demo-status">
            <span className="state-pill state-pill--ready">Accepted</span>
            <span>Stokes · coherent SI · affine MINI/P1</span>
          </div>
        </header>
        <div className="cylinder-demo-evidence-grid">
          <Evidence
            label="Exact → realized geometry"
            primary={`${result.geometry.circleSegments} chords · bound ${scientific(
              result.geometry.boundaryErrorBoundM,
            )} m`}
            secondary={`source ${shortDigest(result.geometry.exactSourceDigest)} · mesh ${shortDigest(
              result.context.meshDigest,
            )}`}
          />
          <Evidence
            label="Cylinder reaction"
            primary={vector(result.cylinderReaction.forceOnFluidNM, "N/m")}
            secondary="constraint force on fluid"
          />
          <Evidence
            label="Signed flux balance"
            primary={`${scientific(result.fluxBalance.netM2S)} m²/s net`}
            secondary={`in ${scientific(result.fluxBalance.inletM2S)} · out ${scientific(
              result.fluxBalance.outletM2S,
            )}`}
          />
          <Evidence
            label="Global momentum closure"
            primary={vector(result.momentumBalance.closureNM, "N/m")}
            secondary="reaction + body force + traction"
          />
          <Evidence
            label="Sparse-LU true residual"
            primary={`${scientific(result.solver.trueResidualNorm)} ≤ ${scientific(
              result.solver.residualTarget,
            )}`}
            secondary={`rtol ${scientific(result.solver.relativeTolerance)} · continuity ${scientific(
              result.solver.continuityResidualNorm,
            )}`}
          />
        </div>
        <p className="cylinder-demo-boundary">
          This is a bounded steady Stokes demonstration—not a Navier–Stokes, drag-coefficient,
          vortex-shedding, or benchmark-comparison claim.
        </p>
      </section>
      <UnstructuredFieldWorkspace
        coordinates={field.coordinates}
        descriptor={field.descriptor}
        onSelect={onSelect}
        selectedVertex={selectedVertex}
        stale={false}
        triangles={field.triangles}
        values={field.values}
      />
    </div>
  );
}

function Evidence({
  label,
  primary,
  secondary,
}: Readonly<{ label: string; primary: string; secondary: string }>) {
  return (
    <article>
      <span>{label}</span>
      <strong>{primary}</strong>
      <small>{secondary}</small>
    </article>
  );
}

function shortDigest(digest: string): string {
  return digest.slice(0, 12);
}

function scientific(value: number): string {
  return value.toExponential(3);
}

function vector(value: readonly [number, number], unit: string): string {
  return `[${scientific(value[0])}, ${scientific(value[1])}] ${unit}`;
}
