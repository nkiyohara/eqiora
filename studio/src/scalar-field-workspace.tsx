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
import type { ScalarFieldDescriptor } from "./scalar-field-protocol";
import "./scalar-field-workspace.css";

const TABLE_PAGE_SIZE = 50;

type ScalarFieldLocation = ScalarFieldDescriptor["field"]["location"];

export interface ScalarFieldWorkspaceProps {
  readonly descriptor: ScalarFieldDescriptor;
  readonly values: Float64Array;
  readonly selectedOrdinal: number;
  readonly onSelect: (ordinal: number) => void;
  readonly stale: boolean;
  readonly realizationRevision: number;
}

type GridIndices = Readonly<{ i: number; j: number }>;
type PhysicalPoint = Readonly<{ xM: number; yM: number }>;

function clamp(value: number, lower: number, upper: number): number {
  return Math.min(Math.max(value, lower), upper);
}

/** Canonical entity ordinal for a last-axis-fastest two-dimensional grid. */
export function scalarFieldOrdinal(
  shape: readonly [number, number],
  i: number,
  j: number,
): number | null {
  const [nx, ny] = shape;
  if (!Number.isInteger(i) || !Number.isInteger(j) || i < 0 || j < 0 || i >= nx || j >= ny) {
    return null;
  }
  return i * ny + j;
}

/** Canonical two-dimensional indices for a last-axis-fastest entity ordinal. */
export function scalarFieldIndices(
  shape: readonly [number, number],
  ordinal: number,
): GridIndices | null {
  const [nx, ny] = shape;
  const count = nx * ny;
  if (
    !Number.isSafeInteger(count) ||
    !Number.isInteger(ordinal) ||
    ordinal < 0 ||
    ordinal >= count
  ) {
    return null;
  }
  return { i: Math.floor(ordinal / ny), j: ordinal % ny };
}

/** Exact vertex or cell-centre coordinate associated with one canonical ordinal. */
export function scalarFieldCoordinates(
  descriptor: Pick<ScalarFieldDescriptor, "domain" | "field" | "grid">,
  ordinal: number,
): PhysicalPoint | null {
  const indices = scalarFieldIndices(descriptor.grid.logicalShape, ordinal);
  if (indices === null) return null;
  const [nx, ny] = descriptor.grid.logicalShape;
  const [[xLower, xUpper], [yLower, yUpper]] = descriptor.domain.boundsM;
  const fraction = (index: number, extent: number) =>
    descriptor.field.location === "vertex"
      ? extent === 1
        ? 0.5
        : index / (extent - 1)
      : (index + 0.5) / extent;
  return {
    xM: xLower + fraction(indices.i, nx) * (xUpper - xLower),
    yM: yLower + fraction(indices.j, ny) * (yUpper - yLower),
  };
}

/** Map a normalized physical viewport point to its nearest exact field entity. */
export function scalarFieldOrdinalAtPoint(
  descriptor: Pick<ScalarFieldDescriptor, "field" | "grid">,
  xFraction: number,
  yFraction: number,
): number {
  const [nx, ny] = descriptor.grid.logicalShape;
  const entityIndex = (fraction: number, extent: number) => {
    const bounded = clamp(fraction, 0, 1);
    return descriptor.field.location === "vertex"
      ? Math.round(bounded * Math.max(extent - 1, 0))
      : Math.min(Math.floor(bounded * extent), extent - 1);
  };
  return scalarFieldOrdinal(
    descriptor.grid.logicalShape,
    entityIndex(xFraction, nx),
    entityIndex(yFraction, ny),
  ) as number;
}

function formatNumber(value: number): string {
  if (value === 0) return "0";
  const magnitude = Math.abs(value);
  if (magnitude >= 1.0e4 || magnitude < 1.0e-3) return value.toExponential(4);
  return Number(value.toPrecision(6)).toLocaleString();
}

function formatFieldValue(value: number | undefined): string {
  return value !== undefined && Number.isFinite(value) ? formatNumber(value) : "Unavailable";
}

function formatBytes(bytes: number): string {
  if (bytes < 1_024) return `${bytes} B`;
  if (bytes < 1_024 * 1_024) return `${(bytes / 1_024).toFixed(1)} KiB`;
  return `${(bytes / (1_024 * 1_024)).toFixed(1)} MiB`;
}

function fieldLocationLabel(location: ScalarFieldLocation): string {
  return location === "vertex" ? "Vertex" : "Cell centre";
}

function entityLabel(location: ScalarFieldLocation): string {
  return location === "vertex" ? "Vertex" : "Cell";
}

function selectedSummary(
  descriptor: ScalarFieldDescriptor,
  values: Float64Array,
  ordinal: number,
): string {
  const indices = scalarFieldIndices(descriptor.grid.logicalShape, ordinal);
  const point = scalarFieldCoordinates(descriptor, ordinal);
  const value = values[ordinal];
  if (indices === null || point === null || value === undefined || !Number.isFinite(value)) {
    return "Field selection is outside the accepted projection.";
  }
  return `${entityLabel(descriptor.field.location)} ${indices.i}, ${indices.j}; x ${formatNumber(
    point.xM,
  )} m; y ${formatNumber(point.yM)} m; value ${formatNumber(value)} ${
    descriptor.field.coherentSiUnit
  }`;
}

type Rgb = readonly [number, number, number];
type ColorStop = readonly [number, Rgb];

const VIRIDIS_START: ColorStop = [0, [68, 1, 84]];
const VIRIDIS_END: ColorStop = [1, [253, 231, 37]];
const VIRIDIS_STOPS: readonly ColorStop[] = [
  VIRIDIS_START,
  [0.25, [59, 82, 139]],
  [0.5, [33, 145, 140]],
  [0.75, [94, 201, 98]],
  VIRIDIS_END,
];

function scalarColor(value: number, minimum: number, maximum: number): string {
  const normalized =
    minimum === maximum ? 0.5 : clamp((value - minimum) / (maximum - minimum), 0, 1);
  const upperIndex = VIRIDIS_STOPS.findIndex(([position]) => normalized <= position);
  const endIndex = upperIndex <= 0 ? 1 : upperIndex;
  const [startPosition, start] = VIRIDIS_STOPS[endIndex - 1] ?? VIRIDIS_START;
  const [endPosition, end] = VIRIDIS_STOPS[endIndex] ?? VIRIDIS_END;
  const amount = (normalized - startPosition) / Math.max(endPosition - startPosition, 1);
  const channel = (index: number) =>
    Math.round((start[index] ?? 0) + ((end[index] ?? 0) - (start[index] ?? 0)) * amount);
  return `rgb(${channel(0)} ${channel(1)} ${channel(2)})`;
}

function requireDrawableContract(descriptor: ScalarFieldDescriptor, values: Float64Array): void {
  const [nx, ny] = descriptor.grid.logicalShape;
  const expected = nx * ny;
  if (
    !Number.isSafeInteger(expected) ||
    nx < 1 ||
    ny < 1 ||
    expected !== descriptor.field.valueCount ||
    values.length !== expected
  ) {
    throw new Error("Field values do not match the accepted two-dimensional logical shape.");
  }
  if (
    values.some((value) => !Number.isFinite(value)) ||
    !Number.isFinite(descriptor.field.minimum) ||
    !Number.isFinite(descriptor.field.maximum) ||
    descriptor.field.maximum < descriptor.field.minimum
  ) {
    throw new Error("Field rendering requires one finite, ordered value range.");
  }
}

function drawField(
  canvas: HTMLCanvasElement,
  descriptor: ScalarFieldDescriptor,
  values: Float64Array,
): void {
  requireDrawableContract(descriptor, values);
  const rectangle = canvas.getBoundingClientRect();
  if (rectangle.width <= 0 || rectangle.height <= 0) return;
  const ratio = Math.max(window.devicePixelRatio, 1);
  const width = Math.max(1, Math.round(rectangle.width * ratio));
  const height = Math.max(1, Math.round(rectangle.height * ratio));
  if (canvas.width !== width || canvas.height !== height) {
    canvas.width = width;
    canvas.height = height;
  }
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("The browser did not provide a Canvas2D renderer.");
  context.save();
  context.setTransform(ratio, 0, 0, ratio, 0, 0);
  context.clearRect(0, 0, rectangle.width, rectangle.height);
  context.fillStyle = "#0d130f";
  context.fillRect(0, 0, rectangle.width, rectangle.height);

  const [nx, ny] = descriptor.grid.logicalShape;
  if (descriptor.field.location === "cell-center") {
    const cellWidth = rectangle.width / nx;
    const cellHeight = rectangle.height / ny;
    for (let i = 0; i < nx; i += 1) {
      for (let j = 0; j < ny; j += 1) {
        const ordinal = scalarFieldOrdinal(descriptor.grid.logicalShape, i, j) as number;
        context.fillStyle = scalarColor(
          values[ordinal] as number,
          descriptor.field.minimum,
          descriptor.field.maximum,
        );
        context.fillRect(
          i * cellWidth,
          rectangle.height - (j + 1) * cellHeight,
          Math.ceil(cellWidth + 0.35),
          Math.ceil(cellHeight + 0.35),
        );
      }
    }
  } else {
    context.fillStyle = "rgba(255, 255, 255, 0.025)";
    context.fillRect(0, 0, rectangle.width, rectangle.height);
    const spacingX = nx === 1 ? rectangle.width : rectangle.width / (nx - 1);
    const spacingY = ny === 1 ? rectangle.height : rectangle.height / (ny - 1);
    const marker = clamp(Math.min(spacingX, spacingY) * 0.72, 2, 12);
    for (let i = 0; i < nx; i += 1) {
      for (let j = 0; j < ny; j += 1) {
        const ordinal = scalarFieldOrdinal(descriptor.grid.logicalShape, i, j) as number;
        const x = nx === 1 ? rectangle.width / 2 : i * spacingX;
        const y = ny === 1 ? rectangle.height / 2 : rectangle.height - j * spacingY;
        context.fillStyle = scalarColor(
          values[ordinal] as number,
          descriptor.field.minimum,
          descriptor.field.maximum,
        );
        context.fillRect(x - marker / 2, y - marker / 2, marker, marker);
      }
    }
  }
  context.restore();
}

function selectedCursorStyle(descriptor: ScalarFieldDescriptor, ordinal: number): CSSProperties {
  const indices = scalarFieldIndices(descriptor.grid.logicalShape, ordinal);
  if (indices === null) return { left: "50%", top: "50%" };
  const [nx, ny] = descriptor.grid.logicalShape;
  const fraction = (index: number, extent: number) =>
    descriptor.field.location === "vertex"
      ? extent === 1
        ? 0.5
        : index / (extent - 1)
      : (index + 0.5) / extent;
  const x = fraction(indices.i, nx) * 100;
  const y = (1 - fraction(indices.j, ny)) * 100;
  return {
    left: `clamp(14px, ${x}%, calc(100% - 14px))`,
    top: `clamp(14px, ${y}%, calc(100% - 14px))`,
  };
}

function nextKeyboardOrdinal(
  descriptor: ScalarFieldDescriptor,
  ordinal: number,
  event: KeyboardEvent,
): number | null {
  const indices = scalarFieldIndices(descriptor.grid.logicalShape, ordinal);
  if (indices === null) return null;
  const [nx, ny] = descriptor.grid.logicalShape;
  let { i, j } = indices;
  switch (event.key) {
    case "ArrowLeft":
      i = Math.max(0, i - 1);
      break;
    case "ArrowRight":
      i = Math.min(nx - 1, i + 1);
      break;
    case "ArrowUp":
      j = Math.min(ny - 1, j + 1);
      break;
    case "ArrowDown":
      j = Math.max(0, j - 1);
      break;
    case "Home":
      if (event.ctrlKey || event.metaKey) i = 0;
      j = 0;
      break;
    case "End":
      if (event.ctrlKey || event.metaKey) i = nx - 1;
      j = ny - 1;
      break;
    default:
      return null;
  }
  return scalarFieldOrdinal(descriptor.grid.logicalShape, i, j);
}

function FieldValueTable({
  descriptor,
  values,
  selectedOrdinal,
  onSelect,
}: Omit<ScalarFieldWorkspaceProps, "realizationRevision" | "stale">) {
  const [page, setPage] = useState(() => Math.floor(selectedOrdinal / TABLE_PAGE_SIZE));
  const pendingKeyboardFocus = useRef<number | null>(null);
  const pageCount = Math.max(1, Math.ceil(descriptor.field.valueCount / TABLE_PAGE_SIZE));
  const boundedPage = clamp(page, 0, pageCount - 1);
  const start = boundedPage * TABLE_PAGE_SIZE;
  const end = Math.min(start + TABLE_PAGE_SIZE, descriptor.field.valueCount);
  const visibleSelected = selectedOrdinal >= start && selectedOrdinal < end;
  const tabOrdinal = visibleSelected ? selectedOrdinal : start;

  useEffect(() => {
    const selectedPage = Math.floor(selectedOrdinal / TABLE_PAGE_SIZE);
    setPage(clamp(selectedPage, 0, pageCount - 1));
    if (pendingKeyboardFocus.current === selectedOrdinal) {
      window.requestAnimationFrame(() => {
        document.getElementById(`field-value-${selectedOrdinal}`)?.focus();
        pendingKeyboardFocus.current = null;
      });
    }
  }, [pageCount, selectedOrdinal]);

  const selectFromKeyboard = (ordinal: number) => {
    pendingKeyboardFocus.current = ordinal;
    setPage(Math.floor(ordinal / TABLE_PAGE_SIZE));
    onSelect(ordinal);
  };

  return (
    <section
      className="scalar-field-table-pane"
      id="field-value-table"
      aria-labelledby="field-value-table-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Semantic alternative</span>
          <h2 id="field-value-table-heading">Field values</h2>
        </div>
        <span className="state-pill">{descriptor.field.valueCount.toLocaleString()}</span>
      </header>
      <div className="scalar-field-table-wrap">
        <table className="scalar-field-table">
          <caption>
            {descriptor.field.name}, {fieldLocationLabel(descriptor.field.location).toLowerCase()}{" "}
            values in canonical last-axis-fastest order
          </caption>
          <thead>
            <tr>
              <th scope="col">Index</th>
              <th scope="col">i / j</th>
              <th scope="col">x [m]</th>
              <th scope="col">y [m]</th>
              <th scope="col">Value [{descriptor.field.coherentSiUnit}]</th>
            </tr>
          </thead>
          <tbody>
            {Array.from({ length: end - start }, (_, offset) => start + offset).map((ordinal) => {
              const indices = scalarFieldIndices(descriptor.grid.logicalShape, ordinal);
              const point = scalarFieldCoordinates(descriptor, ordinal);
              if (indices === null || point === null) return null;
              const selected = ordinal === selectedOrdinal;
              return (
                <tr className={selected ? "is-selected" : undefined} key={ordinal}>
                  <th scope="row">
                    <button
                      aria-current={selected ? "true" : undefined}
                      id={`field-value-${ordinal}`}
                      onClick={() => onSelect(ordinal)}
                      onKeyDown={(event) => {
                        let next: number | null = null;
                        switch (event.key) {
                          case "ArrowUp":
                            next = Math.max(0, ordinal - 1);
                            break;
                          case "ArrowDown":
                            next = Math.min(descriptor.field.valueCount - 1, ordinal + 1);
                            break;
                          case "PageUp":
                            next = Math.max(0, ordinal - TABLE_PAGE_SIZE);
                            break;
                          case "PageDown":
                            next = Math.min(
                              descriptor.field.valueCount - 1,
                              ordinal + TABLE_PAGE_SIZE,
                            );
                            break;
                          case "Home":
                            next =
                              event.ctrlKey || event.metaKey
                                ? 0
                                : Math.floor(ordinal / TABLE_PAGE_SIZE) * TABLE_PAGE_SIZE;
                            break;
                          case "End":
                            next =
                              event.ctrlKey || event.metaKey
                                ? descriptor.field.valueCount - 1
                                : Math.min(
                                    descriptor.field.valueCount - 1,
                                    Math.floor(ordinal / TABLE_PAGE_SIZE) * TABLE_PAGE_SIZE +
                                      TABLE_PAGE_SIZE -
                                      1,
                                  );
                            break;
                        }
                        if (next !== null && next !== ordinal) {
                          event.preventDefault();
                          selectFromKeyboard(next);
                        }
                      }}
                      tabIndex={ordinal === tabOrdinal ? 0 : -1}
                      type="button"
                    >
                      {ordinal}
                    </button>
                  </th>
                  <td>
                    {indices.i} / {indices.j}
                  </td>
                  <td>{formatNumber(point.xM)}</td>
                  <td>{formatNumber(point.yM)}</td>
                  <td>{formatFieldValue(values[ordinal])}</td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <nav className="scalar-field-pagination" aria-label="Field value pages">
        <button
          className="scalar-field-page-button"
          disabled={boundedPage === 0}
          onClick={() => setPage(Math.max(0, boundedPage - 1))}
          type="button"
        >
          Previous
        </button>
        <span aria-live="polite">
          Rows {start + 1}–{end} of {descriptor.field.valueCount.toLocaleString()}
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

function FieldViewport({
  descriptor,
  values,
  selectedOrdinal,
  onSelect,
}: Omit<ScalarFieldWorkspaceProps, "realizationRevision" | "stale">) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const [renderError, setRenderError] = useState<string | null>(null);
  const selectionLabel = selectedSummary(descriptor, values, selectedOrdinal);

  const render = useCallback(() => {
    const element = canvas.current;
    if (element === null) return;
    try {
      drawField(element, descriptor, values);
      setRenderError(null);
    } catch (error: unknown) {
      const previousWidth = element.width;
      element.width = previousWidth;
      setRenderError(error instanceof Error ? error.message : "Field renderer failed.");
    }
  }, [descriptor, values]);

  useEffect(() => {
    const element = canvas.current;
    if (element === null) return;
    render();
    const observer = new ResizeObserver(render);
    observer.observe(element);
    return () => observer.disconnect();
  }, [render]);

  const pointerSelection = (event: MouseEvent<HTMLButtonElement>) => {
    const rectangle = event.currentTarget.getBoundingClientRect();
    if (rectangle.width <= 0 || rectangle.height <= 0) return;
    onSelect(
      scalarFieldOrdinalAtPoint(
        descriptor,
        (event.clientX - rectangle.left) / rectangle.width,
        1 - (event.clientY - rectangle.top) / rectangle.height,
      ),
    );
  };

  return (
    <section
      className="scalar-field-viewport-pane"
      id="field-viewport"
      aria-labelledby="field-viewport-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Accepted scalar projection</span>
          <h2 id="field-viewport-heading">Field viewport</h2>
        </div>
        <span className="scalar-field-location">
          {fieldLocationLabel(descriptor.field.location)}
        </span>
      </header>
      <fieldset className={`scalar-field-canvas-frame${renderError === null ? "" : " has-error"}`}>
        <legend className="sr-only">{descriptor.field.name} two-dimensional scalar field</legend>
        {/* biome-ignore lint/a11y/noAriaHiddenOnFocusable: the canvas duplicates the exact table and keyboard cursor as pixels only. */}
        <canvas aria-hidden="true" className="scalar-field-canvas" ref={canvas} />
        <button
          aria-label="Select the nearest exact field entity at this pointer position"
          className="scalar-field-hit-target"
          onClick={pointerSelection}
          tabIndex={-1}
          type="button"
        />
        {renderError === null ? (
          <button
            aria-label={`${selectionLabel}. Use arrow keys to select an exact neighbouring entity.`}
            className="scalar-field-cursor"
            onKeyDown={(event) => {
              const next = nextKeyboardOrdinal(descriptor, selectedOrdinal, event);
              if (next !== null && next !== selectedOrdinal) {
                event.preventDefault();
                onSelect(next);
              }
            }}
            style={selectedCursorStyle(descriptor, selectedOrdinal)}
            type="button"
          >
            <span aria-hidden="true" />
          </button>
        ) : (
          <div className="scalar-field-render-error" role="alert">
            <strong>Viewport renderer unavailable</strong>
            <p>{renderError} The semantic values table and accepted evidence remain available.</p>
            <button className="scalar-field-retry" onClick={render} type="button">
              Retry renderer
            </button>
          </div>
        )}
      </fieldset>
      <FieldLegend descriptor={descriptor} />
      <p className="scalar-field-viewport-help">
        Select an exact entity by pointer, keyboard cursor, or the values table. Rendered pixels are
        presentation, not probes or numerical evidence.
      </p>
    </section>
  );
}

function FieldLegend({ descriptor }: { readonly descriptor: ScalarFieldDescriptor }) {
  const { minimum, maximum } = descriptor.field;
  const midpoint = minimum / 2 + maximum / 2;
  const constant = minimum === maximum;
  return (
    <section className="scalar-field-legend" aria-labelledby="scalar-field-legend-heading">
      <div>
        <span className="eyebrow">Linear color scale</span>
        <h3 id="scalar-field-legend-heading">
          {descriptor.field.name} · [{descriptor.field.dimension}]
        </h3>
        <small>Coherent-SI unit {descriptor.field.coherentSiUnit}</small>
      </div>
      <div
        className={`scalar-field-gradient${constant ? " is-constant" : ""}`}
        aria-hidden="true"
      />
      <dl>
        <div>
          <dt>{constant ? "Constant" : "Minimum"}</dt>
          <dd>
            {formatNumber(minimum)} {descriptor.field.coherentSiUnit}
          </dd>
        </div>
        {constant ? null : (
          <>
            <div>
              <dt>Midpoint</dt>
              <dd>
                {formatNumber(midpoint)} {descriptor.field.coherentSiUnit}
              </dd>
            </div>
            <div>
              <dt>Maximum</dt>
              <dd>
                {formatNumber(maximum)} {descriptor.field.coherentSiUnit}
              </dd>
            </div>
          </>
        )}
      </dl>
    </section>
  );
}

function FieldSelectionInspector({
  descriptor,
  values,
  selectedOrdinal,
  stale,
  realizationRevision,
}: Omit<ScalarFieldWorkspaceProps, "onSelect">) {
  const indices = scalarFieldIndices(descriptor.grid.logicalShape, selectedOrdinal);
  const point = scalarFieldCoordinates(descriptor, selectedOrdinal);
  const value = values[selectedOrdinal];
  return (
    <aside
      className="scalar-field-inspector"
      id="field-selection-inspector"
      aria-labelledby="field-selection-inspector-heading"
      tabIndex={-1}
    >
      <header className="scalar-field-pane-heading">
        <div>
          <span className="eyebrow">Accepted native projection</span>
          <h2 id="field-selection-inspector-heading">Selection</h2>
        </div>
        <span className={stale ? "state-pill state-pill--warm" : "state-pill state-pill--ready"}>
          {stale ? "Retained" : "Current"}
        </span>
      </header>
      {indices === null || point === null || value === undefined || !Number.isFinite(value) ? (
        <div className="scalar-field-inspector-empty">
          The selected ordinal is outside this exact Field projection.
        </div>
      ) : (
        <div className="scalar-field-inspector-body">
          <div className="scalar-field-selection-title">
            <span aria-hidden="true">▦</span>
            <div>
              <small>{fieldLocationLabel(descriptor.field.location)}</small>
              <h3>
                {entityLabel(descriptor.field.location)} {indices.i}, {indices.j}
              </h3>
            </div>
          </div>
          <dl className="scalar-field-properties">
            <div>
              <dt>Field</dt>
              <dd>{descriptor.field.name}</dd>
            </div>
            <div>
              <dt>Field ID</dt>
              <dd title={descriptor.field.id}>{descriptor.field.id}</dd>
            </div>
            <div>
              <dt>Domain</dt>
              <dd title={descriptor.domain.id}>{descriptor.domain.id}</dd>
            </div>
            <div>
              <dt>Association</dt>
              <dd>{fieldLocationLabel(descriptor.field.location)}</dd>
            </div>
            <div>
              <dt>Canonical index</dt>
              <dd>{selectedOrdinal}</dd>
            </div>
            <div>
              <dt>Logical index</dt>
              <dd>
                {indices.i} / {indices.j}
              </dd>
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
              Host → WebView · {descriptor.field.valueCount.toLocaleString()} f64 values ·{" "}
              {descriptor.transport.chunkCount} accepted chunks · {formatBytes(values.byteLength)}{" "}
              numeric payload · {descriptor.transport.encoding}
            </p>
            <small>Visualization does not rerun or reverify the accepted solution.</small>
          </section>
          <dl className="scalar-field-lineage">
            <div>
              <dt>Run</dt>
              <dd title={descriptor.runId}>{descriptor.runId}</dd>
            </div>
            <div>
              <dt>Realization</dt>
              <dd>r{realizationRevision}</dd>
            </div>
            <div>
              <dt>Model</dt>
              <dd title={descriptor.modelDigest}>{descriptor.modelDigest.slice(0, 12)}</dd>
            </div>
            <div>
              <dt>Plan</dt>
              <dd title={descriptor.planKey}>{descriptor.planKey.slice(0, 12)}</dd>
            </div>
          </dl>
        </div>
      )}
    </aside>
  );
}

export function ScalarFieldWorkspace({
  descriptor,
  values,
  selectedOrdinal,
  onSelect,
  stale,
  realizationRevision,
}: ScalarFieldWorkspaceProps) {
  const selectionAnnouncement = useMemo(
    () => selectedSummary(descriptor, values, selectedOrdinal),
    [descriptor, selectedOrdinal, values],
  );
  return (
    <section className="scalar-field-workspace" aria-labelledby="scalar-field-workspace-heading">
      <header className="scalar-field-workspace-heading">
        <div>
          <span className="eyebrow">Immutable result · bounded 2D projection</span>
          <h1 id="scalar-field-workspace-heading">{descriptor.field.name}</h1>
        </div>
        <div className="scalar-field-workspace-status">
          <span className={stale ? "state-pill state-pill--warm" : "state-pill state-pill--ready"}>
            {stale ? "Retained" : "Accepted"}
          </span>
          <span>
            {descriptor.grid.logicalShape[0]} × {descriptor.grid.logicalShape[1]}
          </span>
        </div>
      </header>
      <div className="scalar-field-workspace-body">
        <FieldValueTable
          descriptor={descriptor}
          onSelect={onSelect}
          selectedOrdinal={selectedOrdinal}
          values={values}
        />
        <FieldViewport
          descriptor={descriptor}
          onSelect={onSelect}
          selectedOrdinal={selectedOrdinal}
          values={values}
        />
        <FieldSelectionInspector
          descriptor={descriptor}
          realizationRevision={realizationRevision}
          selectedOrdinal={selectedOrdinal}
          stale={stale}
          values={values}
        />
      </div>
      <p className="sr-only" aria-live="polite">
        {selectionAnnouncement}
      </p>
    </section>
  );
}
