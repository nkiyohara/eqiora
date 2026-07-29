import {
  type CSSProperties,
  type KeyboardEvent,
  type MouseEvent,
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import type { UnstructuredFieldDescriptor } from "./unstructured-field-protocol";
import { drawUnstructuredP1Field, normalizedCoordinate } from "./unstructured-field-renderer";

const TABLE_PAGE_SIZE = 100;

export interface UnstructuredFieldWorkspaceProps {
  readonly descriptor: UnstructuredFieldDescriptor;
  readonly coordinates: Float64Array;
  readonly triangles: Uint32Array;
  readonly values: Float64Array;
  readonly selectedVertex: number;
  readonly onSelect: (vertex: number) => void;
  readonly stale: boolean;
}

export function unstructuredVertexCoordinates(
  coordinates: Float64Array,
  vertex: number,
): { xM: number; yM: number } | null {
  if (!Number.isSafeInteger(vertex) || vertex < 0) return null;
  const xM = coordinates[vertex * 2];
  const yM = coordinates[vertex * 2 + 1];
  return xM === undefined || yM === undefined || !Number.isFinite(xM) || !Number.isFinite(yM)
    ? null
    : { xM, yM };
}

export function nearestUnstructuredVertex(
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  normalizedX: number,
  normalizedY: number,
): number {
  const x =
    descriptor.domain.boundsM[0][0] +
    clamp(normalizedX, 0, 1) * (descriptor.domain.boundsM[0][1] - descriptor.domain.boundsM[0][0]);
  const y =
    descriptor.domain.boundsM[1][0] +
    clamp(normalizedY, 0, 1) * (descriptor.domain.boundsM[1][1] - descriptor.domain.boundsM[1][0]);
  let nearest = 0;
  let distance = Number.POSITIVE_INFINITY;
  for (let vertex = 0; vertex < descriptor.mesh.vertexCount; vertex += 1) {
    const point = unstructuredVertexCoordinates(coordinates, vertex);
    if (point === null) continue;
    const candidate = (point.xM - x) ** 2 + (point.yM - y) ** 2;
    if (candidate < distance) {
      distance = candidate;
      nearest = vertex;
    }
  }
  return nearest;
}

export function UnstructuredFieldWorkspace({
  descriptor,
  coordinates,
  triangles,
  values,
  selectedVertex,
  onSelect,
  stale,
}: UnstructuredFieldWorkspaceProps) {
  const announcement = useMemo(
    () => selectionSummary(descriptor, coordinates, values, selectedVertex),
    [coordinates, descriptor, selectedVertex, values],
  );
  return (
    <section className="scalar-field-workspace" aria-labelledby="unstructured-field-heading">
      <header className="scalar-field-workspace-heading">
        <div>
          <span className="eyebrow">Immutable result · affine-triangle P1 projection</span>
          <h1 id="unstructured-field-heading">Scalar Field</h1>
        </div>
        <div className="scalar-field-workspace-status">
          <span className={stale ? "state-pill state-pill--warm" : "state-pill state-pill--ready"}>
            {stale ? "Retained" : "Accepted"}
          </span>
          <span>
            {descriptor.mesh.vertexCount.toLocaleString()} vertices ·{" "}
            {descriptor.mesh.triangleCount.toLocaleString()} triangles
          </span>
        </div>
      </header>
      <div className="scalar-field-workspace-body">
        <VertexTable
          coordinates={coordinates}
          descriptor={descriptor}
          onSelect={onSelect}
          selectedVertex={selectedVertex}
          values={values}
        />
        <TriangleViewport
          coordinates={coordinates}
          descriptor={descriptor}
          onSelect={onSelect}
          selectedVertex={selectedVertex}
          triangles={triangles}
          values={values}
        />
        <VertexInspector
          coordinates={coordinates}
          descriptor={descriptor}
          selectedVertex={selectedVertex}
          stale={stale}
          triangles={triangles}
          values={values}
        />
      </div>
      <p className="sr-only" aria-live="polite">
        {announcement}
      </p>
    </section>
  );
}

function VertexTable({
  descriptor,
  coordinates,
  values,
  selectedVertex,
  onSelect,
}: Omit<UnstructuredFieldWorkspaceProps, "triangles" | "stale">) {
  const [page, setPage] = useState(() => Math.floor(selectedVertex / TABLE_PAGE_SIZE));
  const pendingKeyboardFocus = useRef<number | null>(null);
  const pageCount = Math.max(1, Math.ceil(descriptor.mesh.vertexCount / TABLE_PAGE_SIZE));
  const boundedPage = clamp(page, 0, pageCount - 1);
  const start = boundedPage * TABLE_PAGE_SIZE;
  const end = Math.min(start + TABLE_PAGE_SIZE, descriptor.mesh.vertexCount);
  const visibleSelected = selectedVertex >= start && selectedVertex < end;
  const tabVertex = visibleSelected ? selectedVertex : start;

  useEffect(() => {
    setPage(clamp(Math.floor(selectedVertex / TABLE_PAGE_SIZE), 0, pageCount - 1));
    if (pendingKeyboardFocus.current === selectedVertex) {
      window.requestAnimationFrame(() => {
        document.getElementById(`unstructured-vertex-${selectedVertex}`)?.focus();
        pendingKeyboardFocus.current = null;
      });
    }
  }, [pageCount, selectedVertex]);

  const selectFromKeyboard = (vertex: number) => {
    pendingKeyboardFocus.current = vertex;
    setPage(Math.floor(vertex / TABLE_PAGE_SIZE));
    onSelect(vertex);
  };

  return (
    <section
      className="scalar-field-table-pane"
      id="unstructured-vertex-table"
      aria-labelledby="unstructured-vertex-table-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Semantic alternative</span>
          <h2 id="unstructured-vertex-table-heading">Vertex values</h2>
        </div>
        <span className="state-pill">{descriptor.mesh.vertexCount.toLocaleString()}</span>
      </header>
      <div className="scalar-field-table-wrap">
        <table className="scalar-field-table">
          <caption>Exact P1 values in canonical mesh-vertex order</caption>
          <thead>
            <tr>
              <th scope="col">Vertex</th>
              <th scope="col">x [m]</th>
              <th scope="col">y [m]</th>
              <th scope="col">Value [{descriptor.field.coherentSiUnit}]</th>
            </tr>
          </thead>
          <tbody>
            {Array.from({ length: end - start }, (_, offset) => start + offset).map((vertex) => {
              const point = unstructuredVertexCoordinates(coordinates, vertex);
              if (point === null) return null;
              const selected = vertex === selectedVertex;
              return (
                <tr className={selected ? "is-selected" : undefined} key={vertex}>
                  <th scope="row">
                    <button
                      aria-current={selected ? "true" : undefined}
                      id={`unstructured-vertex-${vertex}`}
                      onClick={() => onSelect(vertex)}
                      onKeyDown={(event) => {
                        const next = nextTableVertex(descriptor.mesh.vertexCount, vertex, event);
                        if (next !== null && next !== vertex) {
                          event.preventDefault();
                          selectFromKeyboard(next);
                        }
                      }}
                      tabIndex={vertex === tabVertex ? 0 : -1}
                      type="button"
                    >
                      {vertex}
                    </button>
                  </th>
                  <td>{formatNumber(point.xM)}</td>
                  <td>{formatNumber(point.yM)}</td>
                  <td>{formatNumber(values[vertex])}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <nav className="scalar-field-pagination" aria-label="Vertex value pages">
        <button
          className="scalar-field-page-button"
          disabled={boundedPage === 0}
          onClick={() => setPage(Math.max(0, boundedPage - 1))}
          type="button"
        >
          Previous
        </button>
        <span aria-live="polite">
          Vertices {start + 1}–{end} of {descriptor.mesh.vertexCount.toLocaleString()}
        </span>
        <button
          className="scalar-field-page-button"
          disabled={boundedPage + 1 >= pageCount}
          onClick={() => setPage(Math.min(pageCount - 1, boundedPage + 1))}
          type="button"
        >
          Next
        </button>
      </nav>
    </section>
  );
}

function TriangleViewport({
  descriptor,
  coordinates,
  triangles,
  values,
  selectedVertex,
  onSelect,
}: Omit<UnstructuredFieldWorkspaceProps, "stale">) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [renderError, setRenderError] = useState<string | null>(null);
  const render = useCallback(() => {
    const element = canvas.current;
    if (element === null) return;
    try {
      drawUnstructuredP1Field(element, descriptor, coordinates, triangles, values);
      setRenderError(null);
    } catch (error: unknown) {
      const previousWidth = element.width;
      element.width = previousWidth;
      setRenderError(error instanceof Error ? error.message : "Triangle renderer failed.");
    }
  }, [coordinates, descriptor, triangles, values]);

  useEffect(() => {
    const element = canvas.current;
    if (element === null) return;
    render();
    const observer = new ResizeObserver(render);
    observer.observe(element);
    return () => observer.disconnect();
  }, [render]);

  return (
    <section
      className="scalar-field-viewport-pane"
      id="unstructured-field-viewport"
      aria-labelledby="unstructured-field-viewport-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Accepted scalar projection</span>
          <h2 id="unstructured-field-viewport-heading">Triangle viewport</h2>
        </div>
        <span className="scalar-field-location">P1 vertex</span>
      </header>
      <fieldset className={`scalar-field-canvas-frame${renderError === null ? "" : " has-error"}`}>
        <legend className="sr-only">Two-dimensional affine-triangle scalar Field</legend>
        {/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: exact values remain in the keyboard table. */}
        <canvas aria-hidden="true" className="scalar-field-canvas" ref={canvas} />
        <button
          aria-label="Select the nearest exact mesh vertex at this pointer position"
          className="scalar-field-hit-target"
          onClick={(event) => pointerSelection(event, descriptor, coordinates, onSelect)}
          tabIndex={-1}
          type="button"
        />
        {renderError === null ? (
          <button
            aria-label={`${selectionSummary(descriptor, coordinates, values, selectedVertex)}. Use arrow keys to move through canonical mesh vertices.`}
            className="scalar-field-cursor"
            onKeyDown={(event) => {
              const next = nextSequentialVertex(descriptor.mesh.vertexCount, selectedVertex, event);
              if (next !== null && next !== selectedVertex) {
                event.preventDefault();
                onSelect(next);
              }
            }}
            style={cursorStyle(descriptor, coordinates, selectedVertex)}
            type="button"
          >
            <span aria-hidden="true" />
          </button>
        ) : (
          <div className="scalar-field-render-error" role="alert">
            <strong>Viewport renderer unavailable</strong>
            <p>{renderError} The exact vertex table and lineage remain available.</p>
            <button className="scalar-field-retry" onClick={render} type="button">
              Retry renderer
            </button>
          </div>
        )}
      </fieldset>
      <FieldLegend descriptor={descriptor} />
      <p className="scalar-field-viewport-help">
        Pixels interpolate only for presentation. Selection resolves to an exact P1 vertex and never
        becomes numerical evidence.
      </p>
    </section>
  );
}

function VertexInspector({
  descriptor,
  coordinates,
  triangles,
  values,
  selectedVertex,
  stale,
}: Omit<UnstructuredFieldWorkspaceProps, "onSelect">) {
  const point = unstructuredVertexCoordinates(coordinates, selectedVertex);
  const value = values[selectedVertex];
  return (
    <aside
      className="scalar-field-inspector"
      id="unstructured-field-inspector"
      aria-labelledby="unstructured-field-inspector-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Accepted native projection</span>
          <h2 id="unstructured-field-inspector-heading">Selection</h2>
        </div>
        <span className={stale ? "state-pill state-pill--warm" : "state-pill state-pill--ready"}>
          {stale ? "Retained" : "Current"}
        </span>
      </header>
      {point === null || value === undefined || !Number.isFinite(value) ? (
        <div className="scalar-field-inspector-empty">
          The selected vertex is outside this exact Field projection.
        </div>
      ) : (
        <div className="scalar-field-inspector-body">
          <div className="scalar-field-selection-title">
            <span aria-hidden="true">△</span>
            <div>
              <small>P1 vertex</small>
              <h3>Vertex {selectedVertex}</h3>
            </div>
          </div>
          <dl className="scalar-field-properties">
            <div>
              <dt>Field ID</dt>
              <dd title={descriptor.field.id}>{descriptor.field.id}</dd>
            </div>
            <div>
              <dt>Domain</dt>
              <dd title={descriptor.domain.id}>{descriptor.domain.id}</dd>
            </div>
            <div>
              <dt>Coordinate</dt>
              <dd>
                x {formatNumber(point.xM)} m · y {formatNumber(point.yM)} m
              </dd>
            </div>
            <div className="scalar-field-property-value">
              <dt>Value</dt>
              <dd>
                {formatNumber(value)} {descriptor.field.coherentSiUnit}
                <small>dimension [{descriptor.field.dimension}]</small>
              </dd>
            </div>
          </dl>
          <section className="scalar-field-transfer" aria-label="Visualization data transfer">
            <strong>Explicit owned copy</strong>
            <p>
              {formatBytes(coordinates.byteLength + triangles.byteLength + values.byteLength)} ·
              f64-le coordinates/values · u32-le connectivity
            </p>
            <small>Visualization does not rerun or reverify the accepted solution.</small>
          </section>
          <dl className="scalar-field-lineage">
            <div>
              <dt>Model</dt>
              <dd title={descriptor.modelDigest}>{descriptor.modelDigest.slice(0, 12)}</dd>
            </div>
            <div>
              <dt>Semantic revision</dt>
              <dd>{descriptor.semanticRevision}</dd>
            </div>
            <div>
              <dt>Realization</dt>
              <dd title={descriptor.realizationDigest}>
                {descriptor.realizationDigest.slice(0, 12)}
              </dd>
            </div>
            <div>
              <dt>Run</dt>
              <dd title={descriptor.runDigest}>{descriptor.runDigest.slice(0, 12)}</dd>
            </div>
            <div>
              <dt>Mesh</dt>
              <dd title={descriptor.meshDigest}>{descriptor.meshDigest.slice(0, 12)}</dd>
            </div>
            <div>
              <dt>Snapshot</dt>
              <dd title={descriptor.snapshotDigest}>{descriptor.snapshotDigest.slice(0, 12)}</dd>
            </div>
          </dl>
        </div>
      )}
    </aside>
  );
}

function pointerSelection(
  event: MouseEvent<HTMLButtonElement>,
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  onSelect: (vertex: number) => void,
): void {
  const rectangle = event.currentTarget.getBoundingClientRect();
  if (rectangle.width <= 0 || rectangle.height <= 0) return;
  onSelect(
    nearestUnstructuredVertex(
      descriptor,
      coordinates,
      (event.clientX - rectangle.left) / rectangle.width,
      1 - (event.clientY - rectangle.top) / rectangle.height,
    ),
  );
}

function cursorStyle(
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  vertex: number,
): CSSProperties {
  const point = unstructuredVertexCoordinates(coordinates, vertex);
  if (point === null) return { display: "none" };
  return {
    left: `${normalizedCoordinate(point.xM, descriptor.domain.boundsM[0]) * 100}%`,
    top: `${(1 - normalizedCoordinate(point.yM, descriptor.domain.boundsM[1])) * 100}%`,
  };
}

function nextSequentialVertex(count: number, vertex: number, event: KeyboardEvent): number | null {
  switch (event.key) {
    case "ArrowLeft":
    case "ArrowUp":
      return Math.max(0, vertex - 1);
    case "ArrowRight":
    case "ArrowDown":
      return Math.min(count - 1, vertex + 1);
    case "Home":
      return 0;
    case "End":
      return count - 1;
    default:
      return null;
  }
}

function nextTableVertex(count: number, vertex: number, event: KeyboardEvent): number | null {
  switch (event.key) {
    case "ArrowUp":
      return Math.max(0, vertex - 1);
    case "ArrowDown":
      return Math.min(count - 1, vertex + 1);
    case "PageUp":
      return Math.max(0, vertex - TABLE_PAGE_SIZE);
    case "PageDown":
      return Math.min(count - 1, vertex + TABLE_PAGE_SIZE);
    case "Home":
      return event.ctrlKey || event.metaKey
        ? 0
        : Math.floor(vertex / TABLE_PAGE_SIZE) * TABLE_PAGE_SIZE;
    case "End":
      return event.ctrlKey || event.metaKey
        ? count - 1
        : Math.min(
            count - 1,
            Math.floor(vertex / TABLE_PAGE_SIZE) * TABLE_PAGE_SIZE + TABLE_PAGE_SIZE - 1,
          );
    default:
      return null;
  }
}

function FieldLegend({ descriptor }: { readonly descriptor: UnstructuredFieldDescriptor }) {
  return (
    <section className="scalar-field-legend" aria-labelledby="unstructured-field-legend-heading">
      <div>
        <span className="eyebrow">Linear color scale</span>
        <h3 id="unstructured-field-legend-heading">P1 scalar · [{descriptor.field.dimension}]</h3>
        <small>Coherent-SI unit {descriptor.field.coherentSiUnit}</small>
      </div>
      <div
        className={`scalar-field-gradient${
          descriptor.field.minimum === descriptor.field.maximum ? " is-constant" : ""
        }`}
        aria-hidden="true"
      />
      <dl>
        <div>
          <dt>Minimum</dt>
          <dd>{formatNumber(descriptor.field.minimum)}</dd>
        </div>
        <div>
          <dt>Maximum</dt>
          <dd>{formatNumber(descriptor.field.maximum)}</dd>
        </div>
      </dl>
    </section>
  );
}

function selectionSummary(
  descriptor: UnstructuredFieldDescriptor,
  coordinates: Float64Array,
  values: Float64Array,
  vertex: number,
): string {
  const point = unstructuredVertexCoordinates(coordinates, vertex);
  const value = values[vertex];
  if (point === null || value === undefined || !Number.isFinite(value)) {
    return "No exact unstructured Field vertex is selected.";
  }
  return `Vertex ${vertex}, x ${formatNumber(point.xM)} metres, y ${formatNumber(
    point.yM,
  )} metres, value ${formatNumber(value)} ${descriptor.field.coherentSiUnit}`;
}

function formatNumber(value: number | undefined): string {
  return value === undefined || !Number.isFinite(value)
    ? "Unavailable"
    : new Intl.NumberFormat("en-GB", {
        maximumSignificantDigits: 7,
      }).format(value);
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_024 * 1_024) return `${(bytes / 1_024).toFixed(1)} KiB`;
  return `${(bytes / (1_024 * 1_024)).toFixed(1)} MiB`;
}

function clamp(value: number, lower: number, upper: number): number {
  return Math.min(upper, Math.max(lower, value));
}
