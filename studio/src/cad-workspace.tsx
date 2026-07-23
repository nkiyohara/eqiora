import { useId, useMemo } from "react";
import type { CadProjection, CadSelectionRequest, CadSemanticEntity } from "./cad-protocol";
import {
  cadAxisSideLabel,
  cadEntityLabel,
  cadSelectionRequest,
  resolveCadSelection,
} from "./cad-workflow";
import "./cad-workspace.css";

interface CadWorkspaceProps {
  readonly projection: CadProjection;
  readonly selection: CadSelectionRequest | null;
  readonly selectionPending?: boolean;
  readonly onRequestSelection: (request: CadSelectionRequest) => void;
}

function metres(value: number): string {
  return `${Number(value.toPrecision(5)).toLocaleString()} m`;
}

export function CadWorkspace({
  projection,
  selection,
  selectionPending = false,
  onRequestSelection,
}: CadWorkspaceProps) {
  const resolution = resolveCadSelection(projection, selection);
  const selectedDomain = resolution.kind === "selected" ? resolution.entity.domainId : null;
  const requestSelection = (domain: string) => {
    const request = cadSelectionRequest(projection, domain);
    if (request !== null) onRequestSelection(request);
  };

  return (
    <section className="cad-workspace" aria-labelledby="cad-workspace-heading">
      <header className="cad-workspace__heading">
        <div>
          <span className="eyebrow">Geometry Realization · exact revision</span>
          <h2 id="cad-workspace-heading">Semantic geometry</h2>
        </div>
        <div className="cad-revision" title={projection.geometryDigest}>
          <span aria-hidden="true" />
          Geometry {projection.geometryDigest.slice(0, 10)}
        </div>
      </header>

      <ol className="cad-operation-strip" aria-label="Accepted CAD realization stages">
        <li>
          <span>01</span>
          <div className="cad-operation-copy">
            <strong>STEP stock</strong>
            <small>{projection.design.sourceUnit} · closed shell</small>
          </div>
        </li>
        <li>
          <span>02</span>
          <div className="cad-operation-copy">
            <strong>Constrained sketch</strong>
            <small>{projection.design.sketch.remainingDegreesOfFreedom} degrees of freedom</small>
          </div>
        </li>
        <li>
          <span>03</span>
          <div className="cad-operation-copy">
            <strong>Positive-Z extrusion</strong>
            <small>{metres(projection.design.extrusion.depthM)} depth</small>
          </div>
        </li>
        <li>
          <span>04</span>
          <div className="cad-operation-copy">
            <strong>Intersection</strong>
            <small>
              {projection.build.intersection.solidCount} solid · {projection.build.repair} repair
            </small>
          </div>
        </li>
      </ol>

      <div className="cad-workspace__body">
        <CadSemanticTable
          entities={projection.entities}
          onSelect={requestSelection}
          selectedDomain={selectedDomain}
          selectionPending={selectionPending}
        />
        <CadViewport
          projection={projection}
          onSelect={requestSelection}
          selectedDomain={selectedDomain}
          selectionPending={selectionPending}
        />
        <CadInspector
          projection={projection}
          resolution={resolution}
          selectionPending={selectionPending}
        />
      </div>
    </section>
  );
}

interface CadSemanticTableProps {
  readonly entities: readonly CadSemanticEntity[];
  readonly selectedDomain: string | null;
  readonly selectionPending: boolean;
  readonly onSelect: (domain: string) => void;
}

export function CadSemanticTable({
  entities,
  selectedDomain,
  selectionPending,
  onSelect,
}: CadSemanticTableProps) {
  return (
    <section
      className="cad-entities"
      aria-labelledby="cad-entities-heading"
      id="cad-domain-table"
      tabIndex={-1}
    >
      <div className="cad-pane-heading">
        <div>
          <span className="eyebrow">Semantic path</span>
          <h3 id="cad-entities-heading">Domains</h3>
        </div>
        <span className="state-pill">{entities.length}</span>
      </div>
      <div className="cad-entity-table-wrap">
        <table className="cad-entity-table">
          <caption className="sr-only">
            Semantic body and boundary Domains in the exact Geometry revision
          </caption>
          <thead>
            <tr>
              <th scope="col">Domain</th>
              <th scope="col">Role</th>
              <th scope="col">Physics</th>
            </tr>
          </thead>
          <tbody>
            {entities.map((entity) => {
              const selected = entity.domainId === selectedDomain;
              return (
                <tr className={selected ? "is-selected" : undefined} key={entity.domainId}>
                  <th scope="row">
                    <button
                      aria-current={selected ? "true" : undefined}
                      className="cad-domain-button"
                      disabled={selectionPending && selected}
                      onClick={() => onSelect(entity.domainId)}
                      title={entity.domainId}
                      type="button"
                    >
                      <span aria-hidden="true" className={`cad-domain-mark is-${entity.kind}`} />
                      <span>
                        <strong>{cadEntityLabel(entity)}</strong>
                        <small>{entity.domainId}</small>
                      </span>
                    </button>
                  </th>
                  <td>{entity.kind === "body" ? "Volume" : cadAxisSideLabel(entity)}</td>
                  <td>
                    {entity.relationIds.length + entity.portIds.length === 0 ? (
                      <span className="cad-muted">—</span>
                    ) : (
                      <span className="cad-physics-count">
                        {entity.relationIds.length} R · {entity.portIds.length} P
                      </span>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

interface CadViewportProps {
  readonly projection: CadProjection;
  readonly selectedDomain: string | null;
  readonly selectionPending: boolean;
  readonly onSelect: (domain: string) => void;
}

type ProjectedPoint = Readonly<{ x: number; y: number; depth: number }>;

function projectVertices(
  vertices: readonly (readonly [number, number, number])[],
): ProjectedPoint[] {
  const minimum = [0, 1, 2].map((axis) => Math.min(...vertices.map((vertex) => vertex[axis] ?? 0)));
  const maximum = [0, 1, 2].map((axis) => Math.max(...vertices.map((vertex) => vertex[axis] ?? 0)));
  const center = minimum.map((value, axis) => (value + (maximum[axis] ?? value)) / 2);
  const extent = Math.max(...maximum.map((value, axis) => value - (minimum[axis] ?? value)), 1e-12);
  return vertices.map(([x, y, z]) => {
    const nx = (x - (center[0] ?? 0)) / extent;
    const ny = (y - (center[1] ?? 0)) / extent;
    const nz = (z - (center[2] ?? 0)) / extent;
    return {
      x: 320 + (nx - ny) * 230,
      y: 205 + (nx + ny) * 112 - nz * 230,
      depth: nx + ny + nz,
    };
  });
}

function trianglePath(
  triangle: CadProjection["triangles"][number],
  vertices: readonly ProjectedPoint[],
): string {
  const points = triangle.vertexIndices.map((index) => vertices[index]);
  if (points.some((point) => point === undefined)) return "";
  return `M ${points.map((point) => `${point?.x.toFixed(2)} ${point?.y.toFixed(2)}`).join(" L ")} Z`;
}

function groupedTriangles(
  triangles: CadProjection["triangles"],
  vertices: readonly ProjectedPoint[],
): ReadonlyArray<
  Readonly<{
    domain: string;
    depth: number;
    x: number;
    y: number;
    triangles: CadProjection["triangles"];
  }>
> {
  const groups = new Map<string, Array<CadProjection["triangles"][number]>>();
  for (const triangle of triangles) {
    const group = groups.get(triangle.domainId) ?? [];
    group.push(triangle);
    groups.set(triangle.domainId, group);
  }
  return [...groups.entries()]
    .map(([domain, group]) => {
      const indices = [...new Set(group.flatMap((triangle) => [...triangle.vertexIndices]))];
      const divisor = Math.max(indices.length, 1);
      return {
        domain,
        triangles: group,
        depth: indices.reduce((sum, index) => sum + (vertices[index]?.depth ?? 0), 0) / divisor,
        x: indices.reduce((sum, index) => sum + (vertices[index]?.x ?? 0), 0) / divisor,
        y: indices.reduce((sum, index) => sum + (vertices[index]?.y ?? 0), 0) / divisor,
      };
    })
    .sort((left, right) => left.depth - right.depth || left.domain.localeCompare(right.domain));
}

export function CadViewport({
  projection,
  selectedDomain,
  selectionPending,
  onSelect,
}: CadViewportProps) {
  const svgId = useId().replaceAll(":", "");
  const floorId = `${svgId}-cad-floor`;
  const shadowId = `${svgId}-cad-shadow`;
  const vertices = useMemo(() => projectVertices(projection.verticesM), [projection.verticesM]);
  const groups = useMemo(
    () => groupedTriangles(projection.triangles, vertices),
    [projection.triangles, vertices],
  );
  const entities = useMemo(
    () => new Map(projection.entities.map((entity) => [entity.domainId, entity])),
    [projection.entities],
  );

  return (
    <section
      className="cad-viewport-pane"
      aria-labelledby="cad-viewport-heading"
      id="cad-viewport"
      tabIndex={-1}
    >
      <div className="cad-pane-heading cad-pane-heading--viewport">
        <div>
          <span className="eyebrow">Tessellated projection</span>
          <h3 id="cad-viewport-heading">Geometry viewport</h3>
        </div>
        <span className="cad-view-label">SI · metre</span>
      </div>
      <div className="cad-viewport-frame">
        <svg
          className="cad-viewport"
          focusable="false"
          preserveAspectRatio="xMidYMid meet"
          role="presentation"
          viewBox="0 0 640 410"
        >
          <defs>
            <linearGradient id={floorId} x1="0" x2="0" y1="0" y2="1">
              <stop offset="0" stopColor="#9ed2b7" stopOpacity="0.055" />
              <stop offset="1" stopColor="#9ed2b7" stopOpacity="0" />
            </linearGradient>
            <filter id={shadowId} height="160%" width="160%" x="-30%" y="-30%">
              <feGaussianBlur stdDeviation="12" />
            </filter>
          </defs>
          <ellipse
            className="cad-viewport__shadow"
            cx="320"
            cy="338"
            fill={`url(#${floorId})`}
            filter={`url(#${shadowId})`}
            rx="205"
            ry="34"
          />
          <g className="cad-viewport__axes">
            <path d="M 52 350 L 113 350" />
            <path d="M 52 350 L 29 326" />
            <path d="M 52 350 L 52 288" />
            <text x="120" y="354">
              X
            </text>
            <text x="15" y="322">
              Y
            </text>
            <text x="47" y="279">
              Z
            </text>
          </g>
          {groups.map((group) => {
            const entity = entities.get(group.domain);
            if (entity === undefined) return null;
            const selected = group.domain === selectedDomain;
            return (
              <g className={`cad-face ${selected ? "is-selected" : ""}`} key={group.domain}>
                {group.triangles.map((triangle) => (
                  <path
                    d={trianglePath(triangle, vertices)}
                    key={triangle.vertexIndices.join(":")}
                  />
                ))}
              </g>
            );
          })}
        </svg>
        <fieldset className="cad-viewport-hotspots">
          <legend className="sr-only">Geometry viewport boundaries</legend>
          {groups.map((group) => {
            const entity = entities.get(group.domain);
            if (entity === undefined) return null;
            const selected = group.domain === selectedDomain;
            const label = `${cadEntityLabel(entity)}, ${cadAxisSideLabel(entity) ?? "boundary"}`;
            return (
              <button
                aria-label={`Select ${label}`}
                aria-pressed={selected}
                className={selected ? "cad-face-hotspot is-selected" : "cad-face-hotspot"}
                disabled={selectionPending && selected}
                key={group.domain}
                onClick={() => onSelect(group.domain)}
                style={{ left: `${(group.x / 640) * 100}%`, top: `${(group.y / 410) * 100}%` }}
                title={label}
                type="button"
              >
                <span aria-hidden="true" />
              </button>
            );
          })}
        </fieldset>
        <p className="cad-viewport-help" id="cad-viewport-help">
          Select a boundary here or in the Domain table. Both submit the same exact request.
        </p>
      </div>
    </section>
  );
}

interface CadInspectorProps {
  readonly projection: CadProjection;
  readonly resolution: ReturnType<typeof resolveCadSelection>;
  readonly selectionPending: boolean;
}

export function CadInspector({ projection, resolution, selectionPending }: CadInspectorProps) {
  const entity = resolution.kind === "selected" ? resolution.entity : null;
  return (
    <aside
      className="cad-inspector"
      aria-labelledby="cad-inspector-heading"
      id="cad-selection-inspector"
      tabIndex={-1}
    >
      <div className="cad-pane-heading">
        <div>
          <span className="eyebrow">Accepted application state</span>
          <h3 id="cad-inspector-heading">Selection</h3>
        </div>
        {selectionPending ? <span className="state-pill state-pill--warm">Resolving</span> : null}
      </div>
      {entity === null ? (
        <div className="cad-inspector__empty">
          <span aria-hidden="true">◇</span>
          <p>
            {resolution.kind === "stale"
              ? "Selection belongs to an earlier Geometry revision."
              : resolution.kind === "missing"
                ? "The exact Domain is absent from this projection."
                : "Choose a body or boundary to inspect its semantic and numerical meaning."}
          </p>
        </div>
      ) : (
        <div className="cad-inspector__body">
          <div className="cad-selection-title">
            <span className={`cad-domain-mark is-${entity.kind}`} aria-hidden="true" />
            <div>
              <span>{entity.kind === "body" ? "Body Domain" : "Boundary Domain"}</span>
              <h4>{cadEntityLabel(entity)}</h4>
            </div>
          </div>
          <dl className="cad-property-list">
            <div>
              <dt>Semantic Domain</dt>
              <dd title={entity.domainId}>{entity.domainId}</dd>
            </div>
            {entity.parentDomainId === null ? null : (
              <div>
                <dt>Parent</dt>
                <dd title={entity.parentDomainId}>{entity.parentDomainId}</dd>
              </div>
            )}
            {entity.axis === null ? null : (
              <div>
                <dt>Orientation</dt>
                <dd>{cadAxisSideLabel(entity)}</dd>
              </div>
            )}
            <div>
              <dt>{entity.kind === "body" ? "Mesh cells" : "Mesh facets"}</dt>
              <dd>{entity.meshEntityCount.toLocaleString()}</dd>
            </div>
          </dl>
          <section className="cad-physical-binding" aria-label="Attached physical meaning">
            <div>
              <span>Relations</span>
              <strong>{entity.relationIds.length}</strong>
            </div>
            <div>
              <span>Boundary ports</span>
              <strong>{entity.portIds.length}</strong>
            </div>
          </section>
          <div className="cad-lineage">
            <span className="sr-only">Exact artifact lineage:</span>
            <span>Model</span>
            <i aria-hidden="true" />
            <span>Geometry</span>
            <i aria-hidden="true" />
            <span>Mesh</span>
          </div>
          <dl className="cad-digests">
            <div>
              <dt>Geometry</dt>
              <dd title={projection.geometryDigest}>{projection.geometryDigest.slice(0, 12)}</dd>
            </div>
            <div>
              <dt>Mesh</dt>
              <dd title={projection.meshDigest}>{projection.meshDigest.slice(0, 12)}</dd>
            </div>
          </dl>
        </div>
      )}
    </aside>
  );
}
