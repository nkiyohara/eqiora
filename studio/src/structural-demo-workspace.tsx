import { useMemo, useState } from "react";
import { STRUCTURAL_SCIENTIFIC_CASE, type StructuralDemoResult } from "./structural-demo-protocol";
import "./structural-demo-workspace.css";

const VIEW_WIDTH = 1_040;
const VIEW_HEIGHT = 680;
const PLOT = { left: 74, top: 60, width: 760, height: 560 } as const;
const VIEW_BOUNDS = { xMin: -0.06, xMax: 2.06, yMin: -0.06, yMax: 1.06 } as const;

type Point = Readonly<{ x: number; y: number }>;

export interface StructuralDemoWorkspaceProps {
  readonly result: StructuralDemoResult;
}

export function StructuralDemoWorkspace({ result }: StructuralDemoWorkspaceProps) {
  const [displayScale, setDisplayScale] = useState(1);
  const [selectedVertex, setSelectedVertex] = useState(result.mesh.vertices.length - 1);
  const view = useMemo(() => projectView(result, displayScale), [displayScale, result]);
  const selected = result.mesh.vertices[selectedVertex] ?? result.mesh.vertices[0];
  const selectedDisplacement =
    result.displacement.valuesM[selectedVertex] ?? result.displacement.valuesM[0];
  if (selected === undefined || selectedDisplacement === undefined) {
    throw new Error("Accepted structural payload omitted its bounded vertex field");
  }

  return (
    <div className="structural-demo">
      <header className="structural-demo__hero">
        <div>
          <span className="eyebrow">Verified structural solve · direct Model</span>
          <h1>A clamped elastic panel, resolved</h1>
          <p>
            One two-component Q1 displacement field stretches the grid away from its fixed left
            edge. The pale square is the original mesh; the luminous grid is the solver result.
          </p>
        </div>
        <dl aria-label="Structural execution summary" className="structural-demo__summary">
          <Metric label="mesh" value="16 × 16 Q1" />
          <Metric label="unknown field" value="u = [uₓ, uᵧ]" />
          <Metric label="execution" value="host · f64 · CG" />
        </dl>
      </header>

      <div className="structural-demo__body">
        <section
          aria-labelledby="structural-view-heading"
          className="structural-demo__viewport-panel"
        >
          <div className="structural-demo__section-heading">
            <div>
              <span className="eyebrow">Original → displaced coordinates</span>
              <h2 id="structural-view-heading">Displacement on the accepted mesh</h2>
            </div>
            <span className="state-pill state-pill--ready">accepted solve</span>
          </div>

          <figure
            aria-describedby="structural-view-description"
            aria-label="Original and displaced structural mesh"
            className="structural-view"
            id="structural-viewport"
            tabIndex={-1}
          >
            <figcaption className="sr-only" id="structural-view-description">
              The original unit-square mesh is shown in pale dashed lines. The displaced mesh uses
              the selected presentation scale. The left boundary remains fixed. A selected vertex is
              synchronized with the numeric readout beneath the view.
            </figcaption>
            <svg aria-hidden="true" role="img" viewBox={`0 0 ${VIEW_WIDTH} ${VIEW_HEIGHT}`}>
              <defs>
                <linearGradient id="structural-panel-glow" x1="0" x2="1" y1="0" y2="0">
                  <stop offset="0%" stopColor="#8fc5aa" stopOpacity="0.08" />
                  <stop offset="100%" stopColor="#79afd2" stopOpacity="0.3" />
                </linearGradient>
                <filter
                  id="structural-selected-glow"
                  height="300%"
                  width="300%"
                  x="-100%"
                  y="-100%"
                >
                  <feGaussianBlur result="blur" stdDeviation="4" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
              </defs>
              <rect
                className="structural-view__frame"
                height={PLOT.height}
                rx="14"
                width={PLOT.width}
                x={PLOT.left}
                y={PLOT.top}
              />
              <path className="structural-view__load-wash" d={view.displacedOutline} />
              {view.originalCells.map((cell) => (
                <path className="structural-view__original-cell" d={cell.path} key={cell.index} />
              ))}
              {view.displacedCells.map((cell) => (
                <path
                  className="structural-view__displaced-cell"
                  d={cell.path}
                  key={cell.index}
                  style={{ fill: cell.fill }}
                />
              ))}
              <line
                className="structural-view__clamp"
                x1={view.clamp[0].x}
                x2={view.clamp[1].x}
                y1={view.clamp[0].y}
                y2={view.clamp[1].y}
              />
              {view.displacedVertices.map((vertex) => (
                <circle
                  className="structural-view__vertex"
                  cx={vertex.point.x}
                  cy={vertex.point.y}
                  key={vertex.index}
                  r={vertex.index === selectedVertex ? 0 : 1.25}
                />
              ))}
              <line
                className="structural-view__selected-vector"
                x1={view.originalVertices[selectedVertex]?.x}
                x2={view.displacedVertices[selectedVertex]?.point.x}
                y1={view.originalVertices[selectedVertex]?.y}
                y2={view.displacedVertices[selectedVertex]?.point.y}
              />
              <circle
                className="structural-view__selected-origin"
                cx={view.originalVertices[selectedVertex]?.x}
                cy={view.originalVertices[selectedVertex]?.y}
                r="4"
              />
              <circle
                className="structural-view__selected"
                cx={view.displacedVertices[selectedVertex]?.point.x}
                cy={view.displacedVertices[selectedVertex]?.point.y}
                filter="url(#structural-selected-glow)"
                r="6"
              />
              <text className="structural-view__label clamp" x="30" y="346">
                FIXED
              </text>
              <text className="structural-view__axis" x="846" y="640">
                x · m
              </text>
              <text className="structural-view__axis" x="78" y="42">
                y · m
              </text>
              <g className="structural-view__legend" transform="translate(866 92)">
                <line className="original" x1="0" x2="38" y1="0" y2="0" />
                <text x="50" y="4">
                  original
                </text>
                <line className="displaced" x1="0" x2="38" y1="32" y2="32" />
                <text x="50" y="36">
                  displaced
                </text>
                <rect fill="url(#structural-panel-glow)" height="12" width="38" x="0" y="58" />
                <text x="50" y="69">
                  uₓ tint
                </text>
              </g>
            </svg>
          </figure>

          <div className="structural-demo__controls">
            <label htmlFor="structural-display-scale">
              <span>Presentation scale</span>
              <strong className="structural-control__value">
                {displayScale.toFixed(2)}× {displayScale === 1 ? "· actual displacement" : ""}
              </strong>
            </label>
            <input
              aria-valuetext={`${displayScale.toFixed(2)} times display scale`}
              id="structural-display-scale"
              max="2"
              min="0"
              onChange={(event) => setDisplayScale(Number(event.currentTarget.value))}
              step="0.05"
              type="range"
              value={displayScale}
            />
            <p>
              Display only: coordinates are drawn as x + scale × u. Solver values and evidence below
              never change.
            </p>
          </div>

          <div className="structural-demo__selection">
            <label htmlFor="structural-selected-vertex">
              <span>Selected mesh vertex</span>
              <strong className="structural-control__value">#{selectedVertex}</strong>
            </label>
            <input
              aria-valuetext={`mesh vertex ${selectedVertex}`}
              id="structural-selected-vertex"
              max={result.mesh.vertices.length - 1}
              min="0"
              onChange={(event) => setSelectedVertex(Number(event.currentTarget.value))}
              step="1"
              type="range"
              value={selectedVertex}
            />
            <dl id="structural-vertex-table" tabIndex={-1}>
              <Readout label="x" unit="m" value={formatValue(selected.coordinatesM[0])} />
              <Readout label="y" unit="m" value={formatValue(selected.coordinatesM[1])} />
              <Readout label="uₓ" unit="m" value={formatValue(selectedDisplacement[0])} />
              <Readout label="uᵧ" unit="m" value={formatValue(selectedDisplacement[1])} />
            </dl>
          </div>
        </section>

        <aside className="structural-demo__evidence">
          <section aria-labelledby="structural-balance-heading" className="structural-card balance">
            <span className="eyebrow">Solver-owned global totals</span>
            <h2 id="structural-balance-heading">Reaction and applied body force</h2>
            <div className="structural-card__vectors">
              <VectorReadout
                label="Constrained reaction"
                value={result.balance.constrainedReactionN}
              />
              <span aria-hidden="true" className="structural-card__opposition">
                ⇄
              </span>
              <VectorReadout
                label="Integrated body force"
                value={result.balance.integratedBodyForceN}
              />
            </div>
            <p>
              These two vectors come directly from the accepted solution. Studio does not recover or
              display boundary traction.
            </p>
          </section>

          <section aria-labelledby="structural-solver-heading" className="structural-card solver">
            <div className="structural-demo__section-heading">
              <div>
                <span className="eyebrow">Execution evidence</span>
                <h2 id="structural-solver-heading">True residual accepted</h2>
              </div>
              <strong>{result.execution.completedIterations} iterations</strong>
            </div>
            <div className="structural-card__residual">
              <span>true residual</span>
              <strong>{formatScientific(result.execution.trueResidualNorm)}</strong>
              <small>target · {formatScientific(result.execution.residualTarget)}</small>
            </div>
            <dl className="structural-card__facts">
              <Fact label="method" value="continuous Galerkin" />
              <Fact label="space" value="Q1 · two components" />
              <Fact label="solver" value="CG · identity" />
              <Fact label="reduction" value="reproducible" />
              <Fact label="assembly" value={`${result.execution.assemblyPackets} packets`} />
              <Fact label="placement" value="one host / one worker" />
            </dl>
          </section>

          <section
            aria-labelledby="structural-lineage-heading"
            className="structural-card lineage"
            id="structural-evidence-inspector"
            tabIndex={-1}
          >
            <span className="eyebrow">Content-addressed lineage</span>
            <h2 id="structural-lineage-heading">Model → Realization → Run</h2>
            <dl>
              <Identity label="Model" value={result.lineage.modelDigest} />
              <Identity label="Realization" value={result.lineage.realizationDigest} />
              <Identity label="Run" value={result.lineage.runDigest} />
            </dl>
            <small>
              This Run has no durable Field output artifact; the bounded native payload remains a
              presentation result.
            </small>
          </section>

          <section
            aria-labelledby="structural-attribution-heading"
            className="structural-card attribution"
          >
            <span className="eyebrow">Scientific attribution</span>
            <h2 id="structural-attribution-heading">{STRUCTURAL_SCIENTIFIC_CASE}</h2>
            <span className="structural-card__verified">registered case · verified</span>
            <p>
              The registered case owns the equations, mixed boundary semantics, analytic
              convergence, balance, and falsifiers. This view only presents its ordinary solver
              path.
            </p>
            <small>
              No stress, strain, traction, validation, nonlinear mechanics, contact, or general
              deformation viewer is claimed.
            </small>
          </section>
        </aside>
      </div>
    </div>
  );
}

function projectView(result: StructuralDemoResult, scale: number) {
  const originalVertices = result.mesh.vertices.map((vertex) => screen(vertex.coordinatesM));
  const displacedVertices = result.mesh.vertices.map((vertex, index) => {
    const displacement = result.displacement.valuesM[index] ?? [0, 0];
    return {
      index,
      point: screen([
        vertex.coordinatesM[0] + scale * displacement[0],
        vertex.coordinatesM[1] + scale * displacement[1],
      ]),
      displacement,
    };
  });
  const maximumUx = Math.max(
    Number.EPSILON,
    ...result.displacement.valuesM.map((value) => Math.abs(value[0])),
  );
  const cells = (vertices: readonly Point[]) =>
    result.mesh.cells.map((cell) => {
      const points = orderPolygon(
        cell.vertices.map((vertex) => vertices[vertex] ?? { x: 0, y: 0 }),
      );
      const averageUx =
        cell.vertices.reduce(
          (sum, vertex) => sum + Math.abs(result.displacement.valuesM[vertex]?.[0] ?? 0),
          0,
        ) / cell.vertices.length;
      const tone = Math.min(1, averageUx / maximumUx);
      return {
        index: cell.index,
        path: polygonPath(points),
        fill: `rgba(${Math.round(111 + 22 * tone)}, ${Math.round(
          169 + 16 * tone,
        )}, ${Math.round(196 + 18 * tone)}, ${0.025 + 0.2 * tone})`,
      };
    });
  const left = displacedVertices.filter(
    (vertex) => result.mesh.vertices[vertex.index]?.coordinatesM[0] === 0,
  );
  const clampPoints = left.map((vertex) => vertex.point).sort((a, b) => a.y - b.y);
  const outlineVertices = result.mesh.vertices
    .map((vertex, index) => ({ vertex, point: displacedVertices[index]?.point }))
    .filter(({ vertex, point }) => point !== undefined && isBoundary(vertex.coordinatesM))
    .map(({ point }) => point as Point);

  return {
    originalVertices,
    displacedVertices,
    originalCells: cells(originalVertices),
    displacedCells: cells(displacedVertices.map((vertex) => vertex.point)),
    clamp: [
      clampPoints[0] ?? { x: 0, y: 0 },
      clampPoints[clampPoints.length - 1] ?? { x: 0, y: 0 },
    ] as const,
    displacedOutline: polygonPath(orderPolygon(outlineVertices)),
  };
}

function screen([x, y]: readonly [number, number]): Point {
  return {
    x: PLOT.left + ((x - VIEW_BOUNDS.xMin) / (VIEW_BOUNDS.xMax - VIEW_BOUNDS.xMin)) * PLOT.width,
    y:
      PLOT.top + (1 - (y - VIEW_BOUNDS.yMin) / (VIEW_BOUNDS.yMax - VIEW_BOUNDS.yMin)) * PLOT.height,
  };
}

function orderPolygon(points: readonly Point[]): Point[] {
  const center = points.reduce(
    (sum, point) => ({ x: sum.x + point.x / points.length, y: sum.y + point.y / points.length }),
    { x: 0, y: 0 },
  );
  return [...points].sort(
    (left, right) =>
      Math.atan2(left.y - center.y, left.x - center.x) -
      Math.atan2(right.y - center.y, right.x - center.x),
  );
}

function polygonPath(points: readonly Point[]): string {
  const first = points[0];
  if (first === undefined) return "";
  return `${points
    .map((point, index) => `${index === 0 ? "M" : "L"} ${point.x.toFixed(3)} ${point.y.toFixed(3)}`)
    .join(" ")} Z`;
}

function isBoundary([x, y]: readonly [number, number]): boolean {
  return x === 0 || x === 1 || y === 0 || y === 1;
}

function Metric({ label, value }: Readonly<{ label: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function Readout({ label, unit, value }: Readonly<{ label: string; unit: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
      <span>{unit}</span>
    </div>
  );
}

function VectorReadout({
  label,
  value,
}: Readonly<{ label: string; value: readonly [number, number] }>) {
  return (
    <div>
      <span>{label}</span>
      <strong>
        [{formatValue(value[0])}, {formatValue(value[1])}]
      </strong>
      <small>N</small>
    </div>
  );
}

function Fact({ label, value }: Readonly<{ label: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function Identity({ label, value }: Readonly<{ label: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>
        <code title={value}>{`${value.slice(0, 12)}…${value.slice(-8)}`}</code>
      </dd>
    </div>
  );
}

function formatValue(value: number): string {
  if (value === 0) return "0";
  return Math.abs(value) < 1e-4 ? value.toExponential(4) : value.toFixed(6);
}

function formatScientific(value: number): string {
  return value.toExponential(3);
}
