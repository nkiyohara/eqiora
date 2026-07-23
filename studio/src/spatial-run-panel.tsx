import { Icon } from "./components";
import type { DocumentProjection } from "./protocol";
import type {
  SpatialConfiguration,
  SpatialConfigurationValidation,
  SpatialPlanStatus,
  SpatialRunStatus,
} from "./spatial-workflow";

interface SpatialRunPanelProps {
  readonly workflow: NonNullable<DocumentProjection["workflows"]["scalarElliptic"]>;
  readonly configuration: SpatialConfiguration;
  readonly validation: SpatialConfigurationValidation;
  readonly realizationRevision: number;
  readonly planStatus: SpatialPlanStatus;
  readonly planCurrent: boolean;
  readonly runStatus: SpatialRunStatus;
  readonly stale: boolean;
  readonly blocked: boolean;
  readonly onMethodEdit: (method: SpatialConfiguration["method"]) => void;
  readonly onNumericEdit: (field: "cellsPerAxis" | "workers", value: string) => void;
  readonly onRun: () => void;
}

function methodLabel(method: SpatialConfiguration["method"]): string {
  return method === "finite-element" ? "Finite element" : "Finite volume";
}

export function SpatialRunPanel({
  workflow,
  configuration,
  validation,
  realizationRevision,
  planStatus,
  planCurrent,
  runStatus,
  stale,
  blocked,
  onMethodEdit,
  onNumericEdit,
  onRun,
}: SpatialRunPanelProps) {
  const active = runStatus.kind === "running";
  const plan = planStatus.kind === "ready" && planCurrent ? planStatus.plan : null;
  const canRun = !stale && !active && !blocked && validation.value !== null && plan !== null;
  const stateLabel =
    runStatus.kind === "running"
      ? "Solving"
      : runStatus.kind === "complete"
        ? "Verified"
        : runStatus.kind === "failed"
          ? "Failed"
          : plan === null
            ? null
            : "Plan accepted";
  const stateTone = ["failed"].includes(runStatus.kind)
    ? "state-pill--warm"
    : runStatus.kind === "running"
      ? "state-pill--running"
      : "state-pill--ready";

  return (
    <section className="run-panel spatial-run-panel" aria-labelledby="spatial-run-heading">
      <div className="section-line">
        <div>
          <span className="eyebrow">Realization intent · r{realizationRevision}</span>
          <h2 id="spatial-run-heading">Scalar elliptic solve</h2>
        </div>
        {stateLabel === null ? null : (
          <span className={`state-pill ${stateTone}`}>{stateLabel}</span>
        )}
      </div>

      <section className="spatial-domain-summary" aria-label="Lowered model requirements">
        <span>{workflow.spatialDimension}D</span>
        <span>{workflow.scalarType}</span>
        <span>{workflow.vectorLayout}</span>
      </section>

      <div className="run-form spatial-run-form">
        <label className="spatial-run-form__method">
          <span className="run-form__label">
            <span>Discretization</span>
            <span>method</span>
          </span>
          <select
            disabled={active}
            onChange={(event) => {
              const method = event.currentTarget.value;
              if (method === "finite-element" || method === "finite-volume") {
                onMethodEdit(method);
              }
            }}
            value={configuration.method}
          >
            <option value="finite-element">Finite element · Q1</option>
            <option value="finite-volume">Finite volume · cell centred</option>
          </select>
        </label>
        <label>
          <span className="run-form__label">
            <span>Cells</span>
            <span>per axis</span>
          </span>
          <input
            aria-describedby={
              validation.errors.cellsPerAxis === null ? undefined : "spatial-cells-error"
            }
            aria-invalid={validation.errors.cellsPerAxis !== null}
            autoComplete="off"
            disabled={active}
            inputMode="numeric"
            onChange={(event) => onNumericEdit("cellsPerAxis", event.currentTarget.value)}
            type="text"
            value={configuration.cellsPerAxis}
          />
          {validation.errors.cellsPerAxis === null ? null : (
            <span className="field-error" id="spatial-cells-error">
              {validation.errors.cellsPerAxis}
            </span>
          )}
        </label>
        <label>
          <span className="run-form__label">
            <span>Workers</span>
            <span>≤ {workflow.maximumHostWorkers}</span>
          </span>
          <input
            aria-describedby={
              validation.errors.workers === null ? "worker-budget-note" : "spatial-workers-error"
            }
            aria-invalid={validation.errors.workers !== null}
            autoComplete="off"
            disabled={active}
            inputMode="numeric"
            onChange={(event) => onNumericEdit("workers", event.currentTarget.value)}
            type="text"
            value={configuration.workers}
          />
          {validation.errors.workers === null ? null : (
            <span className="field-error" id="spatial-workers-error">
              {validation.errors.workers}
            </span>
          )}
        </label>
      </div>
      <p className="worker-budget-note" id="worker-budget-note">
        Session budget, resolved once at launch; it is not a hardware claim.
      </p>

      <div className={`capability-preview${active ? " capability-preview--active" : ""}`}>
        {stale ? (
          <p className="capability-preview__message">Compile source changes to resolve a plan.</p>
        ) : validation.value === null ? (
          <p className="capability-preview__message">A bounded Realization intent is required.</p>
        ) : planStatus.kind === "previewing" ? (
          <p className="capability-preview__message" role="status">
            <span className="activity-dot" aria-hidden="true" />
            Resolving capability without allocating the mesh…
          </p>
        ) : planStatus.kind === "failed" ? (
          <p className="capability-preview__message capability-preview__message--error">
            Unsupported. Review the structured diagnostic below.
          </p>
        ) : plan === null ? (
          <p className="capability-preview__message">Waiting for capability preview.</p>
        ) : (
          <>
            <div className="capability-preview__summary spatial-capability-summary">
              <div>
                <span>Numerics</span>
                <strong>{methodLabel(configuration.method)}</strong>
              </div>
              <div>
                <span>Mesh</span>
                <strong>{plan.discretization.cellCount.toLocaleString()} cells</strong>
              </div>
              <div>
                <span>Field</span>
                <strong>{plan.discretization.fieldValueCount.toLocaleString()} values</strong>
              </div>
              <div>
                <span>Placement</span>
                <strong>
                  Host · {plan.placement.workers} worker{plan.placement.workers === 1 ? "" : "s"}
                </strong>
              </div>
            </div>
            <details className="plan-details">
              <summary>Resolved numerical contract</summary>
              <dl>
                <div>
                  <dt>Space / quadrature</dt>
                  <dd>
                    {plan.discretization.space} · {plan.discretization.quadrature}
                  </dd>
                </div>
                <div>
                  <dt>Linear solve</dt>
                  <dd>CG · identity · reproducible reduction</dd>
                </div>
                <div>
                  <dt>Acceptance</dt>
                  <dd>Independent true residual + continuous balance</dd>
                </div>
                <div>
                  <dt>Plan key</dt>
                  <dd>
                    <code title={plan.key}>
                      {plan.key.slice(0, 10)}…{plan.key.slice(-8)}
                    </code>
                  </dd>
                </div>
              </dl>
            </details>
          </>
        )}
      </div>

      {active ? (
        <div className="run-progress spatial-run-progress" role="status" aria-live="polite">
          <div className="run-progress__line">
            <span>Assembly and solve</span>
            <strong>Atomic slice</strong>
          </div>
          <progress aria-label="Spatial solve in progress" />
          <p>No percentage is inferred before accepted solver-observer boundaries exist.</p>
        </div>
      ) : null}

      <div className="run-actions">
        <button
          className="primary-action run-submit"
          disabled={!canRun}
          onClick={onRun}
          type="button"
        >
          <Icon name="play" />
          Assemble, solve, verify
        </button>
      </div>
    </section>
  );
}
