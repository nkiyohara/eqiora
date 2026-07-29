import { useMemo, useState } from "react";
import { DC_MOTOR_SCIENTIFIC_CASE, type DcMotorDemoResult } from "./dc-motor-demo-protocol";
import "./dc-motor-demo-workspace.css";

const CHART_WIDTH = 980;
const CHART_HEIGHT = 520;
const PLOT_LEFT = 86;
const PLOT_RIGHT = 28;
const PLOT_WIDTH = CHART_WIDTH - PLOT_LEFT - PLOT_RIGHT;

type Sample = DcMotorDemoResult["trajectory"]["samples"][number];

export interface DcMotorDemoWorkspaceProps {
  readonly result: DcMotorDemoResult;
}

export function DcMotorDemoWorkspace({ result }: DcMotorDemoWorkspaceProps) {
  const [selectedStep, setSelectedStep] = useState<number>(result.execution.acceptedSteps);
  const selected =
    result.trajectory.samples[selectedStep] ??
    result.trajectory.samples[result.trajectory.samples.length - 1];
  if (selected === undefined) {
    throw new Error("Accepted DC-drive payload omitted its bounded trajectory");
  }
  const governingCommit =
    result.trajectory.commits[
      selectedStep === result.execution.acceptedSteps
        ? result.trajectory.commits.length - 1
        : Math.floor(selectedStep / 10)
    ] ?? result.trajectory.commits[0];
  if (governingCommit === undefined) {
    throw new Error("Accepted DC-drive payload omitted its controller commit ledger");
  }
  const charts = useMemo(() => chartGeometry(result.trajectory.samples), [result]);
  const packages = useMemo(() => orderedPackages(result), [result]);

  return (
    <div className="dc-drive">
      <header className="dc-drive__hero">
        <div>
          <span className="eyebrow">Production trajectory · pinned package closure</span>
          <h1>Sampled DC drive</h1>
          <p>
            A controller commits every 10 ms while the continuous motor advances in 1 ms accepted
            steps.
          </p>
        </div>
        <dl className="dc-drive__summary" aria-label="Execution summary">
          <Metric label="packages" value="3" />
          <Metric label="accepted steps" value="100" />
          <Metric label="held intervals" value="10 × 10 ms" />
        </dl>
      </header>

      <div className="dc-drive__body">
        <section className="dc-drive__trajectory" aria-labelledby="dc-drive-trajectory-heading">
          <div className="dc-drive__section-heading">
            <div>
              <span className="eyebrow">One accepted execution</span>
              <h2 id="dc-drive-trajectory-heading">Current, speed, and held command</h2>
            </div>
            <span className="dc-drive__method">backward Euler · f64 · one host / one worker</span>
          </div>

          <figure
            aria-describedby="dc-drive-chart-description"
            aria-label="DC-drive trajectory"
            className="dc-drive-chart"
            id="trajectory-viewport"
            tabIndex={-1}
          >
            <figcaption className="sr-only" id="dc-drive-chart-description">
              Three aligned plots over 0.1 seconds. Current and angular speed rise continuously.
              Held voltage changes only at the eleven marked controller commits.
            </figcaption>
            <svg aria-hidden="true" role="img" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`}>
              <defs>
                <linearGradient id="dc-current-fill" x1="0" x2="0" y1="0" y2="1">
                  <stop offset="0%" stopColor="#9ed2b7" stopOpacity="0.2" />
                  <stop offset="100%" stopColor="#9ed2b7" stopOpacity="0" />
                </linearGradient>
                <linearGradient id="dc-speed-fill" x1="0" x2="0" y1="0" y2="1">
                  <stop offset="0%" stopColor="#87b8c8" stopOpacity="0.2" />
                  <stop offset="100%" stopColor="#87b8c8" stopOpacity="0" />
                </linearGradient>
              </defs>
              {charts.timeTicks.map((tick) => (
                <g className="dc-drive-chart__time" key={tick.step}>
                  <line x1={tick.x} x2={tick.x} y1="38" y2="474" />
                  <text x={tick.x} y="504">
                    {tick.label}
                  </text>
                </g>
              ))}
              <ChartBand label="CURRENT" top={42} unit="A" />
              <ChartBand label="ANGULAR SPEED" top={205} unit="s⁻¹" />
              <ChartBand label="HELD VOLTAGE" top={368} unit="V" />
              <path className="dc-drive-chart__area current" d={charts.currentArea} />
              <path className="dc-drive-chart__line current" d={charts.currentLine} />
              <path className="dc-drive-chart__area speed" d={charts.speedArea} />
              <path className="dc-drive-chart__line speed" d={charts.speedLine} />
              {result.trajectory.commits.map((commit, ordinal) => (
                <g className="dc-drive-chart__commit" key={commit.step}>
                  <line x1={stepX(commit.step)} x2={stepX(commit.step)} y1="379" y2="470" />
                  <circle cx={stepX(commit.step)} cy="383" r="3.5" />
                  <text x={stepX(commit.step)} y={ordinal % 2 === 0 ? 365 : 357}>
                    {commit.step}
                  </text>
                </g>
              ))}
              <path className="dc-drive-chart__line voltage" d={charts.voltageLine} />
              <line
                className="dc-drive-chart__cursor"
                x1={stepX(selectedStep)}
                x2={stepX(selectedStep)}
                y1="38"
                y2="474"
              />
              <SelectedPoint point={pointAt(charts.currentPoints, selectedStep)} tone="current" />
              <SelectedPoint point={pointAt(charts.speedPoints, selectedStep)} tone="speed" />
              <SelectedPoint point={pointAt(charts.voltagePoints, selectedStep)} tone="voltage" />
              <text className="dc-drive-chart__axis-label" x="932" y="504">
                time · s
              </text>
            </svg>
          </figure>

          <div className="dc-drive__scrubber">
            <label htmlFor="dc-drive-step">
              <span>Selected boundary</span>
              <strong>
                n = {selected.step} · t = {selected.timeS.toFixed(3)} s
              </strong>
            </label>
            <input
              aria-valuetext={`step ${selected.step}, ${selected.timeS.toFixed(3)} seconds`}
              id="dc-drive-step"
              max={result.execution.acceptedSteps}
              min="0"
              onChange={(event) => setSelectedStep(Number(event.currentTarget.value))}
              step="1"
              type="range"
              value={selectedStep}
            />
          </div>

          <dl className="dc-drive__readout" id="trajectory-sample-table">
            <Readout label="Motor current" unit="A" value={formatValue(selected.currentA)} />
            <Readout label="Load speed" unit="s⁻¹" value={formatValue(selected.angularSpeedPerS)} />
            <Readout label="Held command" unit="V" value={formatValue(selected.heldVoltageV)} />
            <Readout
              label={selectedStep === 100 ? "Boundary commit" : "Governing commit"}
              unit={`n = ${governingCommit.step}`}
              value={`${(governingCommit.timeS * 1_000).toFixed(0)} ms`}
            />
          </dl>
        </section>

        <aside className="dc-drive__context">
          <section className="dc-drive__packages" aria-labelledby="dc-drive-packages-heading">
            <div className="dc-drive__section-heading">
              <div>
                <span className="eyebrow">Exact dependency closure</span>
                <h2 id="dc-drive-packages-heading">Three checked-in packages</h2>
              </div>
              <code>{shortDigest(result.packageGraph.resolutionDigest)}</code>
            </div>
            <div className="dc-drive__package-flow">
              {packages.map((node, ordinal) => (
                <article className={ordinal === packages.length - 1 ? "root" : ""} key={node.name}>
                  <span>
                    {ordinal === 0 ? "foundation" : ordinal === 1 ? "drive" : "root model"}
                  </span>
                  <strong>{packageShortName(node.name)}</strong>
                  <small>v{node.version}</small>
                  <code>{shortDigest(node.semanticDigest)}</code>
                  {ordinal < packages.length - 1 ? <i aria-hidden="true">→</i> : null}
                </article>
              ))}
            </div>
            <ul className="dc-drive__edges" aria-label="Package dependency aliases">
              {result.packageGraph.edges.map((edge) => (
                <li key={`${edge.declaring}:${edge.alias}`}>
                  <code>{packageShortName(edge.declaring.split("@")[0] ?? edge.declaring)}</code>
                  <span>uses</span>
                  <strong>{edge.alias}</strong>
                  <span>→</span>
                  <code>{packageShortName(edge.target.split("@")[0] ?? edge.target)}</code>
                </li>
              ))}
            </ul>
          </section>

          <section
            aria-labelledby="dc-drive-lineage-heading"
            className="dc-drive__lineage"
            id="dc-drive-evidence-inspector"
            tabIndex={-1}
          >
            <div className="dc-drive__section-heading">
              <div>
                <span className="eyebrow">Content-addressed lineage</span>
                <h2 id="dc-drive-lineage-heading">Model → Run → binding</h2>
              </div>
              <span className="state-pill state-pill--ready">Bound</span>
            </div>
            <dl>
              <Identity label="Model" value={result.lineage.modelDigest} />
              <Identity label="Compilation" value={result.lineage.compilationDigest} />
              <Identity label="Run" value={result.lineage.runDigest} />
              <Identity label="Package / Run binding" value={result.lineage.runBindingDigest} />
            </dl>
          </section>

          <section className="dc-drive__attribution" aria-labelledby="dc-drive-evidence-heading">
            <span className="eyebrow">Scientific attribution</span>
            <h2 id="dc-drive-evidence-heading">{DC_MOTOR_SCIENTIFIC_CASE}</h2>
            <span className="dc-drive__case-status">registered case · verified</span>
            <p>
              Production trajectory: current, speed, and held (zero-order-hold) voltage command. No
              quantity on this view is recomputed by the application.
            </p>
            <p>
              Energy and power balance for this Model is verified by registered case{" "}
              <code>{DC_MOTOR_SCIENTIFIC_CASE}</code>; it is not recomputed in this payload.
            </p>
            <small>
              Typed physical port samples are retained by the production execution but deliberately
              omitted from this focused presentation.
            </small>
          </section>
        </aside>
      </div>
    </div>
  );
}

function Metric({ label, value }: Readonly<{ label: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

function ChartBand({ label, top, unit }: Readonly<{ label: string; top: number; unit: string }>) {
  return (
    <g className="dc-drive-chart__band">
      <rect height="124" rx="8" width={PLOT_WIDTH} x={PLOT_LEFT} y={top} />
      <text className="label" x="18" y={top + 18}>
        {label}
      </text>
      <text className="unit" x="18" y={top + 36}>
        {unit}
      </text>
    </g>
  );
}

function SelectedPoint({
  point,
  tone,
}: Readonly<{ point: Point; tone: "current" | "speed" | "voltage" }>) {
  return <circle className={`dc-drive-chart__selected ${tone}`} cx={point.x} cy={point.y} r="5" />;
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

function Identity({ label, value }: Readonly<{ label: string; value: string }>) {
  return (
    <div>
      <dt>{label}</dt>
      <dd title={value}>{shortDigest(value)}</dd>
    </div>
  );
}

type Point = Readonly<{ x: number; y: number }>;

function chartGeometry(samples: readonly Sample[]) {
  const currentPoints = points(samples, (sample) => sample.currentA, 50, 158);
  const speedPoints = points(samples, (sample) => sample.angularSpeedPerS, 213, 321);
  const voltagePoints = points(samples, (sample) => sample.heldVoltageV, 376, 464);
  return {
    currentPoints,
    speedPoints,
    voltagePoints,
    currentLine: linePath(currentPoints),
    speedLine: linePath(speedPoints),
    voltageLine: stepPath(voltagePoints),
    currentArea: areaPath(currentPoints, 158),
    speedArea: areaPath(speedPoints, 321),
    timeTicks: [0, 20, 40, 60, 80, 100].map((step) => ({
      step,
      x: stepX(step),
      label: (step / 1_000).toFixed(2),
    })),
  };
}

function points(
  samples: readonly Sample[],
  value: (sample: Sample) => number,
  top: number,
  bottom: number,
): Point[] {
  const values = samples.map(value);
  const minimum = Math.min(...values);
  const maximum = Math.max(...values);
  const span = maximum - minimum || 1;
  return samples.map((sample) => ({
    x: stepX(sample.step),
    y: bottom - ((value(sample) - minimum) / span) * (bottom - top),
  }));
}

function stepX(step: number): number {
  return PLOT_LEFT + (step / 100) * PLOT_WIDTH;
}

function linePath(points: readonly Point[]): string {
  return points.map((point, index) => `${index === 0 ? "M" : "L"}${point.x},${point.y}`).join(" ");
}

function stepPath(points: readonly Point[]): string {
  return points
    .map((point, index) => (index === 0 ? `M${point.x},${point.y}` : `H${point.x} V${point.y}`))
    .join(" ");
}

function areaPath(points: readonly Point[], baseline: number): string {
  const first = points[0];
  const last = points[points.length - 1];
  if (first === undefined || last === undefined) return "";
  return `${linePath(points)} L${last.x},${baseline} L${first.x},${baseline} Z`;
}

function pointAt(points: readonly Point[], index: number): Point {
  const point = points[index] ?? points[points.length - 1];
  if (point === undefined) throw new Error("Accepted DC-drive chart omitted its trajectory points");
  return point;
}

function orderedPackages(result: DcMotorDemoResult) {
  const order = new Map([
    ["Eqiora.Electrical.Basic", 0],
    ["Eqiora.Electromechanical.DcDrive", 1],
    ["org.example.dc_motor_control", 2],
  ]);
  return [...result.packageGraph.nodes].sort(
    (left, right) => (order.get(left.name) ?? 99) - (order.get(right.name) ?? 99),
  );
}

function packageShortName(name: string): string {
  return name.split(".").at(-1)?.replaceAll("_", " ") ?? name;
}

function shortDigest(digest: string): string {
  return `${digest.slice(0, 8)}…${digest.slice(-6)}`;
}

function formatValue(value: number): string {
  return Math.abs(value) < 1.0e-4 && value !== 0 ? value.toExponential(4) : value.toFixed(6);
}
