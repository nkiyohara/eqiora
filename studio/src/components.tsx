import { forwardRef, type ReactNode } from "react";
import type { DocumentProjection, ProjectionNode, ProjectionNodeKind } from "./protocol";
import type { Point, ValueEditStatus } from "./state";
import type { ValueEditValidation } from "./value-edit";

export function Icon({
  name,
}: {
  readonly name: "command" | "compile" | "play" | "reset" | "node";
}) {
  const paths: Record<typeof name, ReactNode> = {
    command: <path d="M5 7h14M5 12h14M5 17h14M8 5v4M16 10v4M11 15v4" />,
    compile: <path d="m7 12 3 3 7-7M5 3h10l4 4v14H5zM15 3v5h5" />,
    play: <path d="m9 7 8 5-8 5z" />,
    reset: <path d="M4 9a8 8 0 1 1 1.2 7.2M4 4v5h5" />,
    node: <path d="M5 6h5v5H5zM14 13h5v5h-5zM10 8.5h4l2.5 4.5" />,
  };
  return (
    <svg aria-hidden="true" className="icon" viewBox="0 0 24 24">
      {paths[name]}
    </svg>
  );
}

interface SourceEditorProps {
  readonly source: string;
  readonly edited: boolean;
  readonly ancestor: boolean;
  readonly onChange: (source: string) => void;
  readonly onCompile: () => void;
}

export const SourceEditor = forwardRef<HTMLTextAreaElement, SourceEditorProps>(
  function SourceEditor({ source, edited, ancestor, onChange, onCompile }, ref) {
    const lineCount = source.split("\n").length;
    const statusLabel = edited
      ? ancestor
        ? "Edited source basis"
        : "Uncompiled changes"
      : ancestor
        ? "Source basis"
        : "In sync";
    return (
      <section className="panel source-panel" aria-labelledby="source-heading">
        <div className="panel-heading">
          <div>
            <span className="eyebrow">Canonical input</span>
            <h2 id="source-heading">Model source</h2>
          </div>
          <span className={edited || ancestor ? "state-pill state-pill--warm" : "state-pill"}>
            {statusLabel}
          </span>
        </div>
        {ancestor ? (
          <p className="source-basis-note">
            Inspector transactions created descendant revisions. Compiling this text starts a new
            source-authored lineage.
          </p>
        ) : null}
        <div className="editor-wrap">
          <textarea
            ref={ref}
            aria-describedby="editor-help"
            aria-label="Eqiora model source"
            className="source-editor"
            onChange={(event) => onChange(event.currentTarget.value)}
            onKeyDown={(event) => {
              if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
                event.preventDefault();
                onCompile();
              }
            }}
            spellCheck={false}
            value={source}
          />
        </div>
        <div className="panel-footer" id="editor-help">
          <span>{lineCount} lines</span>
          <span>
            Compile <kbd>Ctrl</kbd>/<kbd>⌘</kbd> + <kbd>Enter</kbd>
          </span>
        </div>
      </section>
    );
  },
);

const KIND_LABEL: Record<ProjectionNodeKind, string> = {
  domain: "Domain",
  representation: "Representation",
  field: "Field",
  parameter: "Parameter",
  port: "Port",
  relation: "Relation",
  activation: "Activation",
  connection: "Connection",
  "clock-domain": "Clock domain",
};

interface ModelOutlineProps {
  readonly document: DocumentProjection | null;
  readonly selectedNodeId: string | null;
  readonly onSelect: (nodeId: string) => void;
}

export function ModelOutline({ document, selectedNodeId, onSelect }: ModelOutlineProps) {
  return (
    <nav className="outline" aria-label="Model entities">
      <div className="outline__heading">
        <span>Entities</span>
        <span>{document?.nodes.length ?? 0}</span>
      </div>
      {document === null ? (
        <p className="quiet-copy">Compile the source to inspect its canonical entities.</p>
      ) : (
        <ul>
          {document.nodes.map((node) => (
            <li key={node.id}>
              <button
                className={selectedNodeId === node.id ? "outline-item is-selected" : "outline-item"}
                onClick={() => onSelect(node.id)}
                type="button"
              >
                <span aria-hidden="true" className={`entity-dot entity-dot--${node.kind}`} />
                <span>
                  <strong>{node.name}</strong>
                  <small>{KIND_LABEL[node.kind]}</small>
                </span>
              </button>
            </li>
          ))}
        </ul>
      )}
    </nav>
  );
}

interface InspectorProps {
  readonly node: ProjectionNode | null;
  readonly position: Point | null;
  readonly onNudge: (delta: Point) => void;
  readonly valueEdit: Readonly<{
    input: string;
    validation: ValueEditValidation;
    status: ValueEditStatus;
    disabledReason: string | null;
    onChange: (value: string) => void;
    onCommit: () => void;
  }>;
}

export function Inspector({ node, position, onNudge, valueEdit }: InspectorProps) {
  const quantitative =
    node !== null &&
    (node.kind === "field" || node.kind === "parameter") &&
    node.value !== null &&
    node.dimension !== null;
  const plan = valueEdit.status.kind === "ready" ? valueEdit.status.plan : null;
  return (
    <section
      className="inspector"
      aria-labelledby="inspector-heading"
      id="inspector-panel"
      tabIndex={-1}
    >
      <div className="panel-heading panel-heading--compact">
        <div>
          <span className="eyebrow">Selection</span>
          <h2 id="inspector-heading">Inspector</h2>
        </div>
      </div>
      {node === null ? (
        <p className="quiet-copy">Choose an entity in the outline or relation view.</p>
      ) : (
        <div className="inspector__body">
          <div className={`kind-chip kind-chip--${node.kind}`}>{KIND_LABEL[node.kind]}</div>
          <h3>{node.name}</h3>
          <p>{node.summary}</p>
          <dl>
            <div className="inspector__property">
              <dt>Canonical ID</dt>
              <dd title={node.id}>{node.id}</dd>
            </div>
            {node.dimension !== null ? (
              <div className="inspector__property">
                <dt>Dimension</dt>
                <dd>{node.dimension}</dd>
              </div>
            ) : null}
            {node.value !== null ? (
              <div className="inspector__property">
                <dt>Value</dt>
                <dd>{node.value.toLocaleString(undefined, { maximumSignificantDigits: 8 })}</dd>
              </div>
            ) : null}
          </dl>
          {quantitative ? (
            <form
              className="value-editor"
              onSubmit={(event) => {
                event.preventDefault();
                valueEdit.onCommit();
              }}
            >
              <div className="value-editor__heading">
                <div>
                  <span className="eyebrow">Typed transaction</span>
                  <h4>Revision value</h4>
                </div>
                <span className="state-pill">Coherent SI</span>
              </div>
              <label htmlFor="inspector-value-input">Value</label>
              <div className="quantity-input">
                <input
                  aria-describedby="inspector-value-help"
                  aria-invalid={valueEdit.validation.error !== null}
                  disabled={valueEdit.disabledReason !== null}
                  id="inspector-value-input"
                  inputMode="decimal"
                  maxLength={128}
                  onChange={(event) => valueEdit.onChange(event.currentTarget.value)}
                  type="text"
                  value={valueEdit.input}
                />
                <code>[{node.dimension}]</code>
              </div>
              <div aria-live="polite" className="value-editor__status" id="inspector-value-help">
                {valueEdit.disabledReason ??
                  valueEdit.validation.error ??
                  (valueEdit.status.kind === "previewing"
                    ? "Resolving exact transaction…"
                    : valueEdit.status.kind === "committing"
                      ? "Committing atomically…"
                      : plan === null
                        ? "Enter a new scalar to preview the exact graph transaction."
                        : `Revision ${plan.baseRevision} and the current value must still match.`)}
              </div>
              {plan === null ? null : (
                <dl className="transaction-preview">
                  <div>
                    <dt>Change</dt>
                    <dd>
                      {plan.before.value.toPrecision(6)} → {plan.after.value.toPrecision(6)}
                    </dd>
                  </div>
                  <div>
                    <dt>Wire identity</dt>
                    <dd title={plan.transactionDigest}>
                      <code>{plan.transactionDigest.slice(0, 12)}</code>
                    </dd>
                  </div>
                </dl>
              )}
              <button
                className="primary-action value-editor__commit"
                disabled={
                  plan === null ||
                  valueEdit.disabledReason !== null ||
                  valueEdit.status.kind === "committing"
                }
                type="submit"
              >
                {valueEdit.status.kind === "committing" ? "Committing…" : "Commit revision"}
              </button>
            </form>
          ) : null}
          {position === null ? null : (
            <fieldset className="nudge-control">
              <legend>View position</legend>
              <p>Layout is workspace-only and never changes the model.</p>
              <div className="nudge-grid">
                <button
                  aria-label="Move entity up"
                  onClick={() => onNudge({ x: 0, y: -24 })}
                  type="button"
                >
                  ↑
                </button>
                <button
                  aria-label="Move entity left"
                  onClick={() => onNudge({ x: -24, y: 0 })}
                  type="button"
                >
                  ←
                </button>
                <button
                  aria-label="Move entity down"
                  onClick={() => onNudge({ x: 0, y: 24 })}
                  type="button"
                >
                  ↓
                </button>
                <button
                  aria-label="Move entity right"
                  onClick={() => onNudge({ x: 24, y: 0 })}
                  type="button"
                >
                  →
                </button>
              </div>
            </fieldset>
          )}
        </div>
      )}
    </section>
  );
}
