import { type FormEvent, useId, useRef, useState } from "react";
import {
  type CadAuthoredBuildReceipt,
  type CadAuthoredBuildRequest,
  type CadAuthoredFace,
  type CadAuthoredFaceKey,
  type CadAuthoredLineageHandle,
  type CadAuthoredObservations,
  type CadAuthoredOperation,
  type CadAuthoredProjection,
  cadAuthoredBuildRequestSchema,
} from "./cad-authored-protocol";
import {
  type CadAuthoredBridge,
  CadAuthoredSession,
  type CadAuthoredSessionState,
  cadAuthoredBridge,
} from "./cad-authored-session";
import "./cad-authored-workspace.css";

/** Twelve bounded ergonomic scalars; the only authoring surface. */
export interface CadAuthoredFormState {
  readonly xLowerM: string;
  readonly xUpperM: string;
  readonly yLowerM: string;
  readonly yUpperM: string;
  readonly planeZM: string;
  readonly extrusionDepthM: string;
  readonly modelingToleranceM: string;
  readonly cutEnabled: boolean;
  readonly cutCenterXM: string;
  readonly cutCenterYM: string;
  readonly cutRadiusM: string;
  readonly booleanToleranceM: string;
}

// Default scalars are the frozen v2 witness from the accepted case
// `crates/eqiora-geometry/tests/cad_authored_circular_through_cut.rs`.
export const CAD_AUTHORED_DEFAULT_FORM: CadAuthoredFormState = {
  xLowerM: "-0.04",
  xUpperM: "0.04",
  yLowerM: "-0.025",
  yUpperM: "0.025",
  planeZM: "0",
  extrusionDepthM: "0.02",
  modelingToleranceM: "1e-10",
  cutEnabled: true,
  cutCenterXM: "0.02",
  cutCenterYM: "0",
  cutRadiusM: "0.008",
  booleanToleranceM: "1e-9",
};

export type CadAuthoredFormResult =
  | Readonly<{ ok: true; request: CadAuthoredBuildRequest }>
  | Readonly<{ ok: false; message: string }>;

function parseScalar(text: string): number | null {
  const trimmed = text.trim();
  if (trimmed === "") return null;
  const value = Number(trimmed);
  return Number.isFinite(value) ? value : null;
}

/**
 * Turn the ergonomic form into the one closed request shape, or say exactly
 * which field is not a bounded finite scalar. Order, positivity, and cut
 * admission stay with the native owner; the strict schema re-checks bounds.
 */
export function cadAuthoredFormRequest(form: CadAuthoredFormState): CadAuthoredFormResult {
  const scalars = {
    "rectangle x lower": form.xLowerM,
    "rectangle x upper": form.xUpperM,
    "rectangle y lower": form.yLowerM,
    "rectangle y upper": form.yUpperM,
    "sketch plane z": form.planeZM,
    "extrusion depth": form.extrusionDepthM,
    "modeling tolerance": form.modelingToleranceM,
    ...(form.cutEnabled
      ? {
          "cut centre x": form.cutCenterXM,
          "cut centre y": form.cutCenterYM,
          "cut radius": form.cutRadiusM,
          "Boolean tolerance": form.booleanToleranceM,
        }
      : {}),
  };
  const parsed = new Map<string, number>();
  for (const [label, text] of Object.entries(scalars)) {
    const value = parseScalar(text);
    if (value === null) {
      return { ok: false, message: `The ${label} field needs one finite scalar in metres.` };
    }
    parsed.set(label, value);
  }
  const scalar = (label: string): number => parsed.get(label) ?? Number.NaN;
  const request = {
    protocol: "eqiora.studio.cad-authored/v1" as const,
    sketch: {
      xBoundsM: [scalar("rectangle x lower"), scalar("rectangle x upper")] as [number, number],
      yBoundsM: [scalar("rectangle y lower"), scalar("rectangle y upper")] as [number, number],
      planeZM: scalar("sketch plane z"),
    },
    extrusionDepthM: scalar("extrusion depth"),
    requestedModelingToleranceM: scalar("modeling tolerance"),
    cut: form.cutEnabled
      ? {
          centerM: [scalar("cut centre x"), scalar("cut centre y")] as [number, number],
          radiusM: scalar("cut radius"),
          requestedBooleanToleranceM: scalar("Boolean tolerance"),
        }
      : null,
  };
  const checked = cadAuthoredBuildRequestSchema.safeParse(request);
  return checked.success
    ? { ok: true, request: checked.data }
    : {
        ok: false,
        message: checked.error.issues[0]?.message ?? "The request is outside its schema bounds.",
      };
}

const FACE_LABELS: Record<CadAuthoredFaceKey, string> = {
  "start-cap": "Start cap",
  "end-cap": "End cap",
  "profile-x-lower": "X-lower wall",
  "profile-x-upper": "X-upper wall",
  "profile-y-lower": "Y-lower wall",
  "profile-y-upper": "Y-upper wall",
  "cut-wall": "Cut wall",
};

export function cadAuthoredFaceLabel(key: CadAuthoredFaceKey): string {
  return FACE_LABELS[key];
}

/**
 * Every quantity this view names exact is presented as the full
 * `Number.prototype.toString` round-trip of the owner's value — never
 * rounded, truncated, or locale-grouped.
 */
export function formatMetres(value: number): string {
  return `${value.toString()} m`;
}

function formatCount(value: number | null): string {
  return value === null ? "not closed-form" : value.toLocaleString();
}

interface CadAuthoredWorkspaceProps {
  readonly bridge?: CadAuthoredBridge;
}

/**
 * Accessible deterministic inspector and editor over the two admitted
 * authored histories. It renders semantic operations, exact observations,
 * identity, tolerances, and topology lineage — it is not a tessellated
 * renderer and claims nothing beyond the two closed histories.
 */
export function CadAuthoredWorkspace({ bridge = cadAuthoredBridge }: CadAuthoredWorkspaceProps) {
  const [state, setState] = useState<CadAuthoredSessionState>({
    build: { kind: "idle" },
    selection: { kind: "idle" },
  });
  const sessionRef = useRef<CadAuthoredSession | null>(null);
  sessionRef.current ??= new CadAuthoredSession(bridge, setState);
  const session = sessionRef.current;

  const [form, setForm] = useState(CAD_AUTHORED_DEFAULT_FORM);
  const [formError, setFormError] = useState<string | null>(null);

  const submit = (event: FormEvent) => {
    event.preventDefault();
    const parsed = cadAuthoredFormRequest(form);
    if (!parsed.ok) {
      setFormError(parsed.message);
      return;
    }
    setFormError(null);
    void session.build(parsed.request);
  };

  const projection = state.build.kind === "ready" ? state.build.projection : null;
  const selectedHandleHex =
    state.selection.kind === "selected"
      ? state.selection.selection.handleHex
      : state.selection.kind === "resolving"
        ? state.selection.handleHex
        : null;
  const selectionPending = state.selection.kind === "resolving";
  const requestSelection = (handleHex: string) => void session.select(handleHex);

  return (
    <section className="cad-authored-workspace" aria-labelledby="cad-authored-heading">
      <header className="cad-authored-heading">
        <div>
          <span className="eyebrow">Authored operation history · native owner replay</span>
          <h2 id="cad-authored-heading">Authored CAD inspector</h2>
        </div>
        {projection === null ? null : (
          <div className="cad-authored-identity-chip" title={projection.graphDigest}>
            <span aria-hidden="true" />
            Authored graph {projection.graphDigest.slice(0, 10)}
          </div>
        )}
      </header>

      <CadAuthoredControls
        busy={state.build.kind === "building"}
        form={form}
        formError={formError}
        onChange={setForm}
        onSubmit={submit}
      />

      {state.build.kind === "building" ? (
        <p className="cad-authored-status" role="status">
          Replaying the authored history in the native owner…
        </p>
      ) : null}
      {state.build.kind === "failed" ? (
        <div className="cad-authored-error" role="alert">
          <strong>The native owner rejected this history.</strong>
          <p>{state.build.message}</p>
        </div>
      ) : null}
      {state.build.kind === "idle" ? (
        <p className="cad-authored-empty">
          Author a rectangle extrusion — optionally with one circular through-cut — and the native
          Rust owner replays it, returning its exact observations and identity.
        </p>
      ) : null}

      {projection === null ? null : (
        <div className="cad-authored-body">
          <CadAuthoredHistoryPanel history={projection.history} />
          <div className="cad-authored-columns">
            <div className="cad-authored-column">
              <CadAuthoredIdentityPanel projection={projection} />
              <CadAuthoredObservationsPanel observations={projection.observations} />
            </div>
            <CadAuthoredFaceList
              faces={projection.faces}
              onSelect={requestSelection}
              selectedHandleHex={selectedHandleHex}
              selectionPending={selectionPending}
            />
            <div className="cad-authored-column">
              <CadAuthoredSelectionInspector state={state} />
              <CadAuthoredBuildReceiptPanel
                build={projection.build}
                onSelect={requestSelection}
                selectedHandleHex={selectedHandleHex}
                selectionPending={selectionPending}
              />
            </div>
          </div>
        </div>
      )}

      <footer className="cad-authored-claims">
        <p>
          This inspector demonstrates exactly two admitted histories: the frozen rectangle extrusion
          and its one circular through-cut. Every value shown is an exact native owner observation;
          no quantity on this view is recomputed by the application.
        </p>
      </footer>
    </section>
  );
}

interface CadAuthoredControlsProps {
  readonly form: CadAuthoredFormState;
  readonly formError: string | null;
  readonly busy: boolean;
  readonly onChange: (form: CadAuthoredFormState) => void;
  readonly onSubmit: (event: FormEvent) => void;
}

export function CadAuthoredControls({
  form,
  formError,
  busy,
  onChange,
  onSubmit,
}: CadAuthoredControlsProps) {
  const prefix = useId();
  const field = (
    label: string,
    key: keyof Omit<CadAuthoredFormState, "cutEnabled">,
    disabled = false,
  ) => {
    const id = `${prefix}-${key}`;
    return (
      <div className="cad-authored-field">
        <label htmlFor={id}>{label}</label>
        <input
          disabled={disabled || busy}
          id={id}
          inputMode="decimal"
          onChange={(event) => onChange({ ...form, [key]: event.target.value })}
          type="text"
          value={form[key]}
        />
      </div>
    );
  };

  return (
    <form
      aria-labelledby={`${prefix}-controls`}
      className="cad-authored-controls"
      onSubmit={onSubmit}
    >
      <h3 className="sr-only" id={`${prefix}-controls`}>
        Authored history scalars
      </h3>
      <fieldset>
        <legend>Rectangle sketch and extrusion (metres)</legend>
        <div className="cad-authored-field-grid">
          {field("X lower", "xLowerM")}
          {field("X upper", "xUpperM")}
          {field("Y lower", "yLowerM")}
          {field("Y upper", "yUpperM")}
          {field("Sketch plane z", "planeZM")}
          {field("Extrusion depth", "extrusionDepthM")}
          {field("Modeling tolerance τ", "modelingToleranceM")}
        </div>
      </fieldset>
      <fieldset>
        <legend>Circular through-cut</legend>
        <div className="cad-authored-field cad-authored-field--toggle">
          <input
            checked={form.cutEnabled}
            disabled={busy}
            id={`${prefix}-cutEnabled`}
            onChange={(event) => onChange({ ...form, cutEnabled: event.target.checked })}
            type="checkbox"
          />
          <label htmlFor={`${prefix}-cutEnabled`}>Append one circular through-cut</label>
        </div>
        <div className="cad-authored-field-grid">
          {field("Centre x", "cutCenterXM", !form.cutEnabled)}
          {field("Centre y", "cutCenterYM", !form.cutEnabled)}
          {field("Radius", "cutRadiusM", !form.cutEnabled)}
          {field("Boolean tolerance", "booleanToleranceM", !form.cutEnabled)}
        </div>
      </fieldset>
      <div className="cad-authored-controls-footer">
        <button disabled={busy} type="submit">
          Replay in native owner
        </button>
        {formError === null ? null : (
          <p className="cad-authored-form-error" role="alert">
            {formError}
          </p>
        )}
      </div>
    </form>
  );
}

export function CadAuthoredHistoryPanel({
  history,
}: Readonly<{ history: readonly CadAuthoredOperation[] }>) {
  return (
    <ol aria-label="Accepted authored operations in order" className="cad-authored-history">
      {history.map((operation, index) => (
        <li key={operation.id}>
          <span aria-hidden="true">{String(index + 1).padStart(2, "0")}</span>
          <div className="cad-authored-history-copy">
            <strong>{operationTitle(operation)}</strong>
            <small>{operationDetail(operation)}</small>
          </div>
        </li>
      ))}
    </ol>
  );
}

function operationTitle(operation: CadAuthoredOperation): string {
  switch (operation.kind) {
    case "sketch-plane":
      return "XY sketch plane";
    case "rectangle-profile":
      return "Constrained rectangle";
    case "closed-face":
      return "Closed profile face";
    case "positive-z-extrusion":
      return "Positive-Z extrusion";
    case "cut-sketch-plane":
      return "On-face cut sketch plane";
    case "circle-profile":
      return "Constrained circle";
    case "closed-cut-face":
      return "Closed cut face";
    case "circular-through-cut":
      return "Circular through-cut";
  }
}

function operationDetail(operation: CadAuthoredOperation): string {
  switch (operation.kind) {
    case "sketch-plane":
      return `z = ${formatMetres(operation.zM)}`;
    case "rectangle-profile":
      return `x ${formatMetres(operation.xBoundsM[0])} … ${formatMetres(operation.xBoundsM[1])} · y ${formatMetres(operation.yBoundsM[0])} … ${formatMetres(operation.yBoundsM[1])}`;
    case "closed-face":
      return "closed by construction · 1 region";
    case "positive-z-extrusion":
      return `${formatMetres(operation.depthM)} depth · ${operation.repair} repair`;
    case "cut-sketch-plane":
      return `on face ${operation.face}`;
    case "circle-profile":
      return `centre (${formatMetres(operation.centerM[0])}, ${formatMetres(operation.centerM[1])}) · radius ${formatMetres(operation.radiusM)} · closed by construction`;
    case "closed-cut-face":
      return "closed by construction · 1 region";
    case "circular-through-cut":
      return `tool ${operation.toolFace} on ${operation.target} · tolerance ${formatMetres(operation.requestedBooleanToleranceM)} · through all`;
  }
}

export function CadAuthoredIdentityPanel({
  projection,
}: Readonly<{ projection: CadAuthoredProjection }>) {
  return (
    <section aria-labelledby="cad-authored-identity-heading" className="cad-authored-panel">
      <div className="cad-authored-panel-heading">
        <span className="eyebrow">Identity and tolerance</span>
        <h3 id="cad-authored-identity-heading">Authored graph identity</h3>
      </div>
      <dl className="cad-authored-property-list">
        <div>
          <dt>Authored graph digest</dt>
          <dd title={projection.graphDigest}>
            <code>{projection.graphDigest.slice(0, 16)}</code>
          </dd>
        </div>
        <div>
          <dt>Canonical bytes</dt>
          <dd>{projection.canonicalByteCount.toLocaleString()} bytes · opaque</dd>
        </div>
        <div>
          <dt>Requested modeling tolerance τ</dt>
          <dd>{formatMetres(projection.tolerances.requestedModelingToleranceM)}</dd>
        </div>
        {projection.tolerances.requestedBooleanToleranceM === null ? null : (
          <div>
            <dt>Requested Boolean tolerance</dt>
            <dd>{formatMetres(projection.tolerances.requestedBooleanToleranceM)}</dd>
          </div>
        )}
        <div>
          <dt>Repair</dt>
          <dd>{projection.tolerances.repair}</dd>
        </div>
      </dl>
      <p className="cad-authored-note">
        The digest is the authored graph identity, not a Geometry identity: replaying with only a
        different τ keeps every observation on this page while changing the digest, because τ is
        part of the authored meaning and never a coordinate offset.
      </p>
    </section>
  );
}

export function CadAuthoredObservationsPanel({
  observations,
}: Readonly<{ observations: CadAuthoredObservations }>) {
  return (
    <section aria-labelledby="cad-authored-observations-heading" className="cad-authored-panel">
      <div className="cad-authored-panel-heading">
        <span className="eyebrow">Exact analytic observations</span>
        <h3 id="cad-authored-observations-heading">Owner observations</h3>
      </div>
      <dl className="cad-authored-property-list">
        <div>
          <dt>Volume</dt>
          <dd>{observations.volumeM3.toString()} m³</dd>
        </div>
        <div>
          <dt>Surface area</dt>
          <dd>{observations.surfaceAreaM2.toString()} m²</dd>
        </div>
        <div>
          <dt>Faces · shells · bodies</dt>
          <dd>
            {observations.faceCount} · {observations.closedShellCount} · {observations.bodyCount}
          </dd>
        </div>
        <div>
          <dt>Genus</dt>
          <dd>{observations.genus}</dd>
        </div>
        <div>
          <dt>Vertices</dt>
          <dd>{formatCount(observations.vertexCount)}</dd>
        </div>
        <div>
          <dt>Edges</dt>
          <dd>{formatCount(observations.edgeCount)}</dd>
        </div>
        <div>
          <dt>Outer bounds</dt>
          <dd>
            {observations.boundsM
              .map(([lower, upper], axis) => `${"xyz"[axis] ?? "?"}: ${lower} … ${upper}`)
              .join(" · ")}
          </dd>
        </div>
      </dl>
    </section>
  );
}

interface CadAuthoredFaceListProps {
  readonly faces: readonly CadAuthoredFace[];
  readonly selectedHandleHex: string | null;
  readonly selectionPending: boolean;
  readonly onSelect: (handleHex: string) => void;
}

export function CadAuthoredFaceList({
  faces,
  selectedHandleHex,
  selectionPending,
  onSelect,
}: CadAuthoredFaceListProps) {
  return (
    <section aria-labelledby="cad-authored-faces-heading" className="cad-authored-panel">
      <div className="cad-authored-panel-heading">
        <span className="eyebrow">Admitted provenance</span>
        <h3 id="cad-authored-faces-heading">Faces</h3>
        <span className="state-pill">{faces.length}</span>
      </div>
      <ul className="cad-authored-face-list">
        {faces.map((face) => {
          const selected = face.handleHex === selectedHandleHex;
          return (
            <li key={face.provenanceKey}>
              <button
                aria-pressed={selected}
                className={selected ? "cad-authored-face is-selected" : "cad-authored-face"}
                disabled={selectionPending && selected}
                onClick={() => onSelect(face.handleHex)}
                type="button"
              >
                <span className="cad-authored-face-name">
                  <strong>{cadAuthoredFaceLabel(face.provenanceKey)}</strong>
                  <small>{face.provenanceKey}</small>
                </span>
                <span className="cad-authored-face-meta">
                  {face.areaM2.toString()} m² · {face.boundaryLoopCount}{" "}
                  {face.boundaryLoopCount === 1 ? "loop" : "loops"}
                </span>
              </button>
            </li>
          );
        })}
      </ul>
      <p className="cad-authored-note">
        Selecting a face sends its opaque graph-bound handle back to the native owner for replay;
        the owner rejects any stale, foreign, or cross-version handle before resolving.
      </p>
    </section>
  );
}

export function CadAuthoredSelectionInspector({
  state,
}: Readonly<{ state: CadAuthoredSessionState }>) {
  const selection = state.selection;
  return (
    <aside aria-labelledby="cad-authored-selection-heading" className="cad-authored-panel">
      <div className="cad-authored-panel-heading">
        <span className="eyebrow">Accepted application state</span>
        <h3 id="cad-authored-selection-heading">Selection</h3>
        {selection.kind === "resolving" ? (
          <span className="state-pill state-pill--warm" role="status">
            Resolving
          </span>
        ) : null}
      </div>
      {selection.kind === "failed" ? (
        <p className="cad-authored-error" role="alert">
          {selection.message}
        </p>
      ) : null}
      {selection.kind === "selected" ? (
        <dl className="cad-authored-property-list">
          <div>
            <dt>Provenance</dt>
            <dd>{cadAuthoredFaceLabel(selection.selection.provenanceKey)}</dd>
          </div>
          <div>
            <dt>Face area</dt>
            <dd>{selection.selection.areaM2.toString()} m²</dd>
          </div>
          <div>
            <dt>Boundary loops</dt>
            <dd>{selection.selection.boundaryLoopCount}</dd>
          </div>
          {selection.selection.centroidM === null ? null : (
            <div>
              <dt>Centroid</dt>
              <dd>({selection.selection.centroidM.map(formatMetres).join(", ")})</dd>
            </div>
          )}
          {selection.selection.outwardUnitNormal === null ? null : (
            <div>
              <dt>Outward normal</dt>
              <dd>({selection.selection.outwardUnitNormal.join(", ")})</dd>
            </div>
          )}
          <div>
            <dt>Bound to graph</dt>
            <dd title={selection.selection.graphDigest}>
              <code>{selection.selection.graphDigest.slice(0, 16)}</code>
            </dd>
          </div>
        </dl>
      ) : null}
      {selection.kind === "idle" ? (
        <p className="cad-authored-empty">
          Choose a face from the list or from the lineage below; both send the same exact handle.
        </p>
      ) : null}
    </aside>
  );
}

interface CadAuthoredBuildReceiptPanelProps {
  readonly build: CadAuthoredBuildReceipt;
  readonly selectedHandleHex: string | null;
  readonly selectionPending: boolean;
  readonly onSelect: (handleHex: string) => void;
}

const LINEAGE_LABELS = [
  ["retainedUnchanged", "Retained unchanged"],
  ["retainedModified", "Retained · new inner loop"],
  ["created", "Created"],
  ["deleted", "Deleted"],
  ["split", "Split"],
  ["merged", "Merged"],
] as const;

export function CadAuthoredBuildReceiptPanel({
  build,
  selectedHandleHex,
  selectionPending,
  onSelect,
}: CadAuthoredBuildReceiptPanelProps) {
  return (
    <section aria-labelledby="cad-authored-build-heading" className="cad-authored-panel">
      <div className="cad-authored-panel-heading">
        <span className="eyebrow">Execution evidence · not identity</span>
        <h3 id="cad-authored-build-heading">Analytic build receipt</h3>
      </div>
      <dl className="cad-authored-property-list">
        <div>
          <dt>Provider profile</dt>
          <dd>
            <code>{build.providerProfile}</code>
          </dd>
        </div>
        {build.effectiveBooleanToleranceM === null ? null : (
          <div>
            <dt>Effective Boolean tolerance</dt>
            <dd>{formatMetres(build.effectiveBooleanToleranceM)} · no substitution</dd>
          </div>
        )}
        <div>
          <dt>Max discrepancies</dt>
          <dd>
            {build.maximumPositionDiscrepancyM} m · {build.maximumAreaDiscrepancyM2} m² ·{" "}
            {build.maximumVolumeDiscrepancyM3} m³
          </dd>
        </div>
        <div>
          <dt>Repair</dt>
          <dd>{build.repair}</dd>
        </div>
      </dl>
      {LINEAGE_LABELS.map(([key, label]) => (
        <CadAuthoredLineageRow
          entries={build.lineage[key]}
          key={key}
          label={label}
          onSelect={onSelect}
          selectedHandleHex={selectedHandleHex}
          selectionPending={selectionPending}
        />
      ))}
    </section>
  );
}

interface CadAuthoredLineageRowProps {
  readonly label: string;
  readonly entries: readonly CadAuthoredLineageHandle[];
  readonly selectedHandleHex: string | null;
  readonly selectionPending: boolean;
  readonly onSelect: (handleHex: string) => void;
}

function CadAuthoredLineageRow({
  label,
  entries,
  selectedHandleHex,
  selectionPending,
  onSelect,
}: CadAuthoredLineageRowProps) {
  if (entries.length === 0) return null;
  return (
    <div className="cad-authored-lineage-row">
      <span className="cad-authored-lineage-label">{label}</span>
      <ul aria-label={`${label} faces`} className="cad-authored-lineage-chips">
        {entries.map((entry) => {
          const selected = entry.handleHex === selectedHandleHex;
          return (
            <li key={entry.provenanceKey}>
              <button
                aria-pressed={selected}
                className={
                  selected ? "cad-authored-lineage-chip is-selected" : "cad-authored-lineage-chip"
                }
                disabled={selectionPending && selected}
                onClick={() => onSelect(entry.handleHex)}
                type="button"
              >
                {cadAuthoredFaceLabel(entry.provenanceKey)}
              </button>
            </li>
          );
        })}
      </ul>
    </div>
  );
}
