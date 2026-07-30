import { type CSSProperties, useMemo, useState } from "react";
import type { FsiDemoResult } from "./fsi-demo-protocol";
import "./fsi-demo-workspace.css";

const DISPLAY_SCALE = 12;
const VIEW = { left: 72, bottom: 352, widthPerM: 342, heightPerM: 268 } as const;

function engineering(value: number, digits = 4): string {
  if (value === 0) return "0";
  const magnitude = Math.abs(value);
  return magnitude >= 1e4 || magnitude < 1e-3
    ? value.toExponential(digits)
    : value.toPrecision(digits);
}

function digestLabel(digest: string): string {
  return `${digest.slice(0, 15)}…${digest.slice(-8)}`;
}

function screenPoint([x, y]: readonly [number, number]): [number, number] {
  return [VIEW.left + x * VIEW.widthPerM, VIEW.bottom - y * VIEW.heightPerM];
}

type Step = FsiDemoResult["steps"][number];
type Vertex = FsiDemoResult["mesh"]["vertices"][number];

function FsiScene({
  result,
  selectedVertex,
  step,
}: Readonly<{
  result: FsiDemoResult;
  selectedVertex: number;
  step: Step;
}>) {
  const pressureByVertex = new Map(
    step.pressure.supportVertices.map((vertex, index) => [
      vertex,
      step.pressure.values[index] ?? 0,
    ]),
  );
  const pressureExtent = Math.max(...step.pressure.values.map((value) => Math.abs(value)), 1e-30);
  const velocityExtent = Math.max(
    ...step.velocity.values.flatMap(([x, y]) => [Math.abs(x), Math.abs(y)]),
    1e-30,
  );
  const actionExtent = Math.max(
    ...step.interfaceActions.flatMap((action) => [
      Math.abs(action.fluid[0]),
      Math.abs(action.fluid[1]),
      Math.abs(action.solid[0]),
      Math.abs(action.solid[1]),
    ]),
    1e-30,
  );
  const displayCoordinates = (vertex: Vertex): [number, number] => {
    if (vertex.coordinatesM[0] < 1) return vertex.coordinatesM;
    const displacement = step.displacement.values[vertex.index] ?? [0, 0];
    return [
      vertex.coordinatesM[0] + DISPLAY_SCALE * displacement[0],
      vertex.coordinatesM[1] + DISPLAY_SCALE * displacement[1],
    ];
  };
  const cellPoints = (vertices: readonly number[], deformed: boolean) =>
    vertices
      .map((index) => {
        const vertex = result.mesh.vertices[index];
        if (vertex === undefined) return "0,0";
        return screenPoint(deformed ? displayCoordinates(vertex) : vertex.coordinatesM).join(",");
      })
      .join(" ");
  const pressureTone = (vertices: readonly number[]): CSSProperties => {
    const pressure =
      vertices.reduce((sum, vertex) => sum + (pressureByVertex.get(vertex) ?? 0), 0) /
      vertices.length;
    const signed = Math.max(-1, Math.min(1, pressure / pressureExtent));
    return {
      "--pressure-alpha": `${0.34 + 0.42 * Math.abs(signed)}`,
      "--pressure-hue": `${signed >= 0 ? 190 : 27}`,
    } as CSSProperties;
  };
  return (
    <figure
      aria-describedby="fsi-scene-description"
      className="fsi-scene"
      id="fsi-viewport"
      tabIndex={-1}
    >
      <figcaption className="sr-only" id="fsi-scene-description">
        Fluid pressure occupies the left body. The right solid is shown at a twelve-times
        presentation scale. One bright vertical interface carries the shared velocity trace and
        opposite solver-owned actions.
      </figcaption>
      <svg aria-hidden="true" viewBox="0 0 840 430">
        <defs>
          <linearGradient id="fsi-fluid-wash" x1="0" x2="1">
            <stop offset="0" stopColor="#12354c" />
            <stop offset="1" stopColor="#0c273b" />
          </linearGradient>
          <linearGradient id="fsi-solid-wash" x1="0" x2="1">
            <stop offset="0" stopColor="#41261f" />
            <stop offset="1" stopColor="#251a20" />
          </linearGradient>
          <filter id="fsi-interface-glow" x="-100%" y="-40%" width="300%" height="180%">
            <feGaussianBlur result="blur" stdDeviation="4" />
            <feMerge>
              <feMergeNode in="blur" />
              <feMergeNode in="SourceGraphic" />
            </feMerge>
          </filter>
          <marker id="fsi-velocity-arrow" markerHeight="6" markerWidth="7" orient="auto" refX="6">
            <path d="M 0 0 L 7 3 L 0 6 z" fill="#7ce4ff" />
          </marker>
          <marker
            id="fsi-fluid-action-arrow"
            markerHeight="6"
            markerWidth="7"
            orient="auto"
            refX="6"
          >
            <path d="M 0 0 L 7 3 L 0 6 z" fill="#68f0c2" />
          </marker>
          <marker
            id="fsi-solid-action-arrow"
            markerHeight="6"
            markerWidth="7"
            orient="auto"
            refX="6"
          >
            <path d="M 0 0 L 7 3 L 0 6 z" fill="#ff9a78" />
          </marker>
        </defs>

        <rect className="fsi-scene__fluid-base" height="268" width="342" x="72" y="84" />
        <rect className="fsi-scene__solid-base" height="268" width="342" x="414" y="84" />

        {result.mesh.cells
          .filter((cell) => cell.region === "fluid")
          .map((cell) => (
            <polygon
              className="fsi-scene__fluid-cell"
              key={`fluid-${cell.index}`}
              points={cellPoints(cell.vertices, false)}
              style={pressureTone(cell.vertices)}
            />
          ))}

        {result.mesh.cells
          .filter((cell) => cell.region === "solid")
          .map((cell) => (
            <polygon
              className="fsi-scene__solid-reference"
              key={`reference-${cell.index}`}
              points={cellPoints(cell.vertices, false)}
            />
          ))}
        {result.mesh.cells
          .filter((cell) => cell.region === "solid")
          .map((cell) => (
            <polygon
              className="fsi-scene__solid-cell"
              key={`solid-${cell.index}`}
              points={cellPoints(cell.vertices, true)}
            />
          ))}

        {result.mesh.interfaceFacets.map((facet) => (
          <line
            className="fsi-scene__interface"
            filter="url(#fsi-interface-glow)"
            key={facet.index}
            x1={screenPoint(result.mesh.vertices[facet.vertices[0]]?.coordinatesM ?? [0, 0])[0]}
            x2={screenPoint(result.mesh.vertices[facet.vertices[1]]?.coordinatesM ?? [0, 0])[0]}
            y1={screenPoint(result.mesh.vertices[facet.vertices[0]]?.coordinatesM ?? [0, 0])[1]}
            y2={screenPoint(result.mesh.vertices[facet.vertices[1]]?.coordinatesM ?? [0, 0])[1]}
          />
        ))}

        {result.mesh.vertices.map((vertex) => {
          const [x, y] = screenPoint(displayCoordinates(vertex));
          const velocity = step.velocity.values[vertex.index] ?? [0, 0];
          const arrowScale = 35 / velocityExtent;
          return (
            <g key={`velocity-${vertex.index}`}>
              {velocity[0] === 0 && velocity[1] === 0 ? null : (
                <line
                  className="fsi-scene__velocity"
                  markerEnd="url(#fsi-velocity-arrow)"
                  x1={x}
                  x2={x + velocity[0] * arrowScale}
                  y1={y}
                  y2={y - velocity[1] * arrowScale}
                />
              )}
            </g>
          );
        })}

        {step.interfaceActions.flatMap((action) => {
          const vertex = result.mesh.vertices[action.vertex];
          if (vertex === undefined) return [];
          const [x, y] = screenPoint(displayCoordinates(vertex));
          const actionScale = 58 / actionExtent;
          return [
            <line
              className="fsi-scene__action fsi-scene__action--fluid"
              key="fluid-action"
              markerEnd="url(#fsi-fluid-action-arrow)"
              x1={x}
              x2={x + action.fluid[0] * actionScale}
              y1={y}
              y2={y - action.fluid[1] * actionScale}
            />,
            <line
              className="fsi-scene__action fsi-scene__action--solid"
              key="solid-action"
              markerEnd="url(#fsi-solid-action-arrow)"
              x1={x}
              x2={x + action.solid[0] * actionScale}
              y1={y}
              y2={y - action.solid[1] * actionScale}
            />,
          ];
        })}

        {result.mesh.vertices.map((vertex) => {
          const [x, y] = screenPoint(displayCoordinates(vertex));
          return (
            <circle
              className={
                selectedVertex === vertex.index
                  ? "fsi-scene__vertex fsi-scene__vertex--selected"
                  : "fsi-scene__vertex"
              }
              cx={x}
              cy={y}
              key={`vertex-${vertex.index}`}
              r={selectedVertex === vertex.index ? 6 : 3.5}
            />
          );
        })}

        <text className="fsi-scene__region-label" x="102" y="56">
          FLUID · MINI/P1
        </text>
        <text className="fsi-scene__region-label" x="615" y="56">
          SOLID · P1
        </text>
        <text className="fsi-scene__interface-label" x="427" y="78">
          one shared trace
        </text>
        <text className="fsi-scene__axis-label" x="72" y="390">
          0 m
        </text>
        <text className="fsi-scene__axis-label" textAnchor="middle" x="414" y="390">
          1 m
        </text>
        <text className="fsi-scene__axis-label" textAnchor="end" x="756" y="390">
          2 m
        </text>
      </svg>
    </figure>
  );
}

function AcceptanceCard({ step }: Readonly<{ step: Step }>) {
  const acceptance = step.physicsAcceptance;
  const checks = [
    ["Reapplied residual", acceptance.numericalResidualNorm, "< 1e−9"],
    ["Weak continuity", acceptance.continuityResidualNorm, "< 1e−9"],
    ["Kinematic closure", acceptance.kinematicResidualNorm, "< 1e−14"],
    ["Shared-trace jump", acceptance.interfaceVelocityJumpNorm, "= 0"],
    ["Action imbalance", acceptance.interfaceActionImbalanceNPerM, "< 1e−9 N/m"],
    ["Energy defect", acceptance.absoluteEnergyDefectJPerM, "< 1e−9 J/m"],
  ] as const;
  return (
    <section className="fsi-card fsi-card--acceptance" aria-labelledby="fsi-physics-title">
      <div className="fsi-card__heading">
        <span aria-hidden="true">✓</span>
        <div>
          <small>Independent acceptance</small>
          <h2 id="fsi-physics-title">Physics closes</h2>
        </div>
      </div>
      <dl className="fsi-checks">
        {checks.map(([label, value, threshold]) => (
          <div key={label}>
            <dt>{label}</dt>
            <dd>
              <strong>{engineering(value)}</strong>
              <span>{threshold}</span>
            </dd>
          </div>
        ))}
      </dl>
    </section>
  );
}

function SolverCard({ result, step }: Readonly<{ result: FsiDemoResult; step: Step }>) {
  return (
    <section className="fsi-card" aria-labelledby="fsi-solver-title">
      <div className="fsi-card__heading">
        <span aria-hidden="true">∿</span>
        <div>
          <small>Backend stopping</small>
          <h2 id="fsi-solver-title">MINRES report</h2>
        </div>
      </div>
      <dl className="fsi-solver-grid">
        <div>
          <dt>True residual</dt>
          <dd>{engineering(step.solverStopping.trueResidualNorm)}</dd>
        </div>
        <div>
          <dt>Solver target</dt>
          <dd>{engineering(step.solverStopping.residualTarget)}</dd>
        </div>
        <div>
          <dt>Iterations</dt>
          <dd>{step.solverStopping.completedIterations}</dd>
        </div>
        <div>
          <dt>Assembly</dt>
          <dd>
            {step.assembly.packetCount} packets · {step.assembly.targetCount} targets
          </dd>
        </div>
      </dl>
      <p>
        Identity preconditioner · reproducible reduction · rtol{" "}
        {result.execution.relativeTolerance.toExponential(0)}
      </p>
    </section>
  );
}

function SelectedVertex({
  result,
  selectedVertex,
  setSelectedVertex,
  step,
}: Readonly<{
  result: FsiDemoResult;
  selectedVertex: number;
  setSelectedVertex: (vertex: number) => void;
  step: Step;
}>) {
  const pressurePosition = step.pressure.supportVertices.indexOf(selectedVertex);
  const pressure = pressurePosition < 0 ? null : step.pressure.values[pressurePosition];
  return (
    <section className="fsi-values" aria-labelledby="fsi-values-title">
      <div className="fsi-section-heading">
        <div>
          <span className="eyebrow">Exact retained coefficients</span>
          <h2 id="fsi-values-title">Vertex {selectedVertex}</h2>
        </div>
        <span>no display scaling</span>
      </div>
      <div className="fsi-selected-values" aria-live="polite">
        <div>
          <small>Velocity · m/s</small>
          <strong>
            [{engineering(step.velocity.values[selectedVertex]?.[0] ?? 0)},{" "}
            {engineering(step.velocity.values[selectedVertex]?.[1] ?? 0)}]
          </strong>
        </div>
        <div>
          <small>Pressure · Pa</small>
          <strong>
            {pressure === null || pressure === undefined
              ? "outside support"
              : engineering(pressure)}
          </strong>
        </div>
        <div>
          <small>Displacement · m</small>
          <strong>
            [{engineering(step.displacement.values[selectedVertex]?.[0] ?? 0)},{" "}
            {engineering(step.displacement.values[selectedVertex]?.[1] ?? 0)}]
          </strong>
        </div>
      </div>
      <div className="fsi-table-scroll">
        <table id="fsi-vertex-table" tabIndex={-1}>
          <caption>
            Exact mesh coordinates and accepted coefficients at {step.timeS.toFixed(2)} seconds
          </caption>
          <thead>
            <tr>
              <th scope="col">Vertex</th>
              <th scope="col">x, y · m</th>
              <th scope="col">vₓ, vᵧ · m/s</th>
              <th scope="col">p · Pa</th>
              <th scope="col">dₓ, dᵧ · m</th>
            </tr>
          </thead>
          <tbody>
            {result.mesh.vertices.map((vertex) => {
              const pressureIndex = step.pressure.supportVertices.indexOf(vertex.index);
              const vertexPressure =
                pressureIndex < 0 ? null : (step.pressure.values[pressureIndex] ?? null);
              const velocity = step.velocity.values[vertex.index] ?? [0, 0];
              const displacement = step.displacement.values[vertex.index] ?? [0, 0];
              return (
                <tr data-selected={selectedVertex === vertex.index} key={vertex.index}>
                  <th scope="row">
                    <button onClick={() => setSelectedVertex(vertex.index)} type="button">
                      {vertex.index}
                    </button>
                  </th>
                  <td>{vertex.coordinatesM.map((value) => engineering(value, 3)).join(", ")}</td>
                  <td>{velocity.map((value) => engineering(value, 3)).join(", ")}</td>
                  <td>{vertexPressure === null ? "—" : engineering(vertexPressure, 3)}</td>
                  <td>{displacement.map((value) => engineering(value, 3)).join(", ")}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

export function FsiDemoWorkspace({ result }: Readonly<{ result: FsiDemoResult }>) {
  const [stepIndex, setStepIndex] = useState(0);
  const [selectedVertex, setSelectedVertex] = useState(3);
  const step = result.steps[stepIndex] ?? result.steps[0];
  if (step === undefined) throw new Error("Accepted FSI result omitted both frozen steps");
  const action = step.interfaceActions[0];
  const energy = step.energy;
  const digestRows = useMemo<readonly (readonly [string, string])[]>(
    () => [
      ["Model", result.lineage.modelDigest],
      ["Geometry", result.lineage.geometryDigest],
      ["Correspondence", result.lineage.correspondenceDigest],
      ["Mesh", result.lineage.meshDigest],
      ["Realization", result.lineage.realizationDigest],
      ["Final Run", result.lineage.runDigest],
      ["State 1", result.lineage.stateDigests[0] ?? ""],
      ["State 2", result.lineage.stateDigests[1] ?? ""],
      ["Trajectory", result.lineage.trajectoryDigest],
    ],
    [result],
  );

  return (
    <article className="fsi-workspace">
      <header className="fsi-hero">
        <div>
          <span className="eyebrow">Verified fixed-reference composition</span>
          <h1>
            One trace. Two bodies.
            <br />
            <em>Two accepted steps.</em>
          </h1>
          <p>
            Inertial Stokes fluid and a dynamic elastic solid meet on one exact conforming
            interface. The native runtime owns every value, balance, and identity below.
          </p>
        </div>
        <div className="fsi-hero__status">
          <span aria-hidden="true" />
          <div>
            <small>Accepted trajectory</small>
            <strong>t = {step.timeS.toFixed(2)} s</strong>
          </div>
        </div>
      </header>

      <nav aria-label="Accepted FSI step" className="fsi-step-switcher">
        {result.steps.map((candidate, index) => (
          <button
            aria-current={index === stepIndex ? "step" : undefined}
            key={candidate.step}
            onClick={() => setStepIndex(index)}
            type="button"
          >
            <span>0{candidate.step}</span>
            <strong>{candidate.timeS.toFixed(2)} s</strong>
            <small>{index === 0 ? "Prestrain released" : "Continued state"}</small>
          </button>
        ))}
        <div className="fsi-step-switcher__line" aria-hidden="true" />
      </nav>

      <section className="fsi-stage">
        <div className="fsi-stage__main">
          <div className="fsi-stage__toolbar">
            <ul className="fsi-legend" aria-label="Scene legend">
              <li className="fluid">Pressure field</li>
              <li className="solid">Displaced solid</li>
              <li className="trace">Shared trace</li>
              <li className="velocity">Velocity</li>
            </ul>
            <span className="fsi-display-scale">solid display ×{DISPLAY_SCALE}</span>
          </div>
          <FsiScene result={result} selectedVertex={selectedVertex} step={step} />
          <div className="fsi-stage__caption">
            <p>
              The luminous interface retains its fixed reference coordinate; the solid outline is
              amplified for legibility. Coordinates and table values remain solver-owned and
              unscaled.
            </p>
            {action === undefined ? null : (
              <p>
                <span className="action-fluid">Fluid action</span> and{" "}
                <span className="action-solid">solid action</span> are opposite weak actions at the
                free midpoint, retained in N/m.
              </p>
            )}
          </div>
        </div>
        <aside className="fsi-stage__evidence">
          <AcceptanceCard step={step} />
          <SolverCard result={result} step={step} />
        </aside>
      </section>

      <SelectedVertex
        result={result}
        selectedVertex={selectedVertex}
        setSelectedVertex={setSelectedVertex}
        step={step}
      />

      <section className="fsi-lower-grid">
        <article className="fsi-energy">
          <div className="fsi-section-heading">
            <div>
              <span className="eyebrow">Backward-Euler identity</span>
              <h2>Energy ledger</h2>
            </div>
            <span>J/m · intrinsic 2D</span>
          </div>
          <dl>
            <div>
              <dt>Previous kinetic</dt>
              <dd>{engineering(energy.previousKinetic)}</dd>
            </div>
            <div>
              <dt>Next kinetic</dt>
              <dd>{engineering(energy.nextKinetic)}</dd>
            </div>
            <div>
              <dt>Previous elastic</dt>
              <dd>{engineering(energy.previousElastic)}</dd>
            </div>
            <div>
              <dt>Next elastic</dt>
              <dd>{engineering(energy.nextElastic)}</dd>
            </div>
            <div>
              <dt>Kinetic increment</dt>
              <dd>{engineering(energy.kineticIncrement)}</dd>
            </div>
            <div>
              <dt>Elastic increment</dt>
              <dd>{engineering(energy.elasticIncrement)}</dd>
            </div>
            <div>
              <dt>Viscous dissipation</dt>
              <dd>{engineering(energy.viscousDissipation)}</dd>
            </div>
            <div className="defect">
              <dt>Identity defect</dt>
              <dd>{engineering(energy.defect)}</dd>
            </div>
          </dl>
        </article>

        <article className="fsi-lineage" id="fsi-evidence-inspector" tabIndex={-1}>
          <div className="fsi-section-heading">
            <div>
              <span className="eyebrow">Content-addressed in memory</span>
              <h2>One immutable lineage</h2>
            </div>
            <span>1 Run output</span>
          </div>
          <dl>
            {digestRows.map(([label, digest]) => (
              <div key={label}>
                <dt>{label}</dt>
                <dd title={digest}>{digestLabel(digest)}</dd>
              </div>
            ))}
          </dl>
        </article>
      </section>

      <footer className="fsi-boundary">
        <div>
          <strong>What this demonstrates</strong>
          <p>
            One fixed 2D mesh, monolithic MINI/P1–P1 coupling, two genuine consecutive reference
            steps, and exact state/trajectory publication.
          </p>
        </div>
        <div>
          <strong>Deliberate boundary</strong>
          <p>
            No ALE motion, advection, remeshing, partitioned coupling, 3D, stress, drag, validation,
            production solver, MPI, GPU, or scale claim.
          </p>
        </div>
        <div className="fsi-authority">
          {result.evidence.map((evidence) => (
            <span key={evidence.caseId}>
              <i aria-hidden="true" /> {evidence.caseId}
            </span>
          ))}
        </div>
      </footer>
    </article>
  );
}
