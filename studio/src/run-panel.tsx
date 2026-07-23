import { Icon } from "./components";
import type { RunConfiguration, RunConfigurationValidation } from "./run-configuration";
import type { RunPlanStatus, RunStatus } from "./state";

interface RunPanelProps {
  readonly digest: string | null;
  readonly stale: boolean;
  readonly configuration: RunConfiguration;
  readonly validation: RunConfigurationValidation;
  readonly status: RunStatus;
  readonly planStatus: RunPlanStatus;
  readonly planCurrent: boolean;
  readonly onEdit: (field: keyof RunConfiguration, value: string) => void;
  readonly onRun: () => void;
  readonly onCancel: () => void;
}

export function RunPanel({
  digest,
  stale,
  configuration,
  validation,
  status,
  planStatus,
  planCurrent,
  onEdit,
  onRun,
  onCancel,
}: RunPanelProps) {
  const active = status.kind === "running" || status.kind === "cancelling";
  const canRun = digest !== null && !stale && validation.value !== null && planCurrent && !active;
  const plan = planStatus.kind === "ready" && planCurrent ? planStatus.plan : null;
  const progress = active ? status.progress : null;
  const completion =
    progress === null ? 0 : Math.min(1, Math.max(0, progress.modelTime / progress.endTime));
  const stateLabel =
    status.kind === "running"
      ? "Running"
      : status.kind === "cancelling"
        ? "Cancelling"
        : status.kind === "cancelled"
          ? "Cancelled"
          : status.kind === "complete"
            ? "Completed"
            : status.kind === "failed"
              ? "Failed"
              : plan === null
                ? null
                : "Plan accepted";
  const stateTone =
    status.kind === "running"
      ? "state-pill--running"
      : ["cancelling", "cancelled", "failed"].includes(status.kind)
        ? "state-pill--warm"
        : "state-pill--ready";
  return (
    <section className="run-panel" aria-labelledby="run-heading">
      <div className="section-line">
        <div>
          <span className="eyebrow">Execution intent</span>
          <h2 id="run-heading">Reference run</h2>
        </div>
        {stateLabel === null ? null : (
          <span className={`state-pill ${stateTone}`}>{stateLabel}</span>
        )}
      </div>
      <div className="run-form">
        <label>
          <span className="run-form__label">
            <span>End time</span>
            <span>s</span>
          </span>
          <input
            aria-describedby={validation.errors.endTime === null ? undefined : "end-time-error"}
            aria-invalid={validation.errors.endTime !== null}
            autoComplete="off"
            disabled={active}
            inputMode="decimal"
            onChange={(event) => onEdit("endTime", event.currentTarget.value)}
            type="text"
            value={configuration.endTime}
          />
          {validation.errors.endTime === null ? null : (
            <span className="field-error" id="end-time-error">
              {validation.errors.endTime}
            </span>
          )}
        </label>
        <label>
          <span className="run-form__label">
            <span>Max step</span>
            <span>s</span>
          </span>
          <input
            aria-describedby={validation.errors.maxStep === null ? undefined : "max-step-error"}
            aria-invalid={validation.errors.maxStep !== null}
            autoComplete="off"
            disabled={active}
            inputMode="decimal"
            onChange={(event) => onEdit("maxStep", event.currentTarget.value)}
            type="text"
            value={configuration.maxStep}
          />
          {validation.errors.maxStep === null ? null : (
            <span className="field-error" id="max-step-error">
              {validation.errors.maxStep}
            </span>
          )}
        </label>
      </div>
      <div
        className={`capability-preview${active ? " capability-preview--active" : ""}`}
        aria-live="polite"
      >
        {stale ? (
          <p className="capability-preview__message">Compile source changes to resolve a plan.</p>
        ) : validation.value === null || digest === null ? (
          <p className="capability-preview__message">Valid model-time inputs are required.</p>
        ) : planStatus.kind === "previewing" ? (
          <p className="capability-preview__message">
            <span className="activity-dot" aria-hidden="true" />
            Checking the requested plan against the native runtime…
          </p>
        ) : planStatus.kind === "failed" ? (
          <p className="capability-preview__message capability-preview__message--error">
            Unsupported. Review the structured diagnostic below.
          </p>
        ) : plan === null ? (
          <p className="capability-preview__message">Waiting for capability preview.</p>
        ) : (
          <>
            <div className="capability-preview__summary">
              <div>
                <span>Adapter</span>
                <strong title={plan.adapter.id}>{plan.adapter.id}</strong>
              </div>
              <div>
                <span>Placement</span>
                <strong>Host · {plan.placement.workers} worker</strong>
              </div>
              <div>
                <span>Integrator</span>
                <strong>Backward Euler</strong>
              </div>
            </div>
            <details className="plan-details">
              <summary>Numerical contract</summary>
              <dl>
                <div>
                  <dt>Nonlinear solve</dt>
                  <dd>Dense finite-difference Newton</dd>
                </div>
                <div>
                  <dt>Residual tolerances</dt>
                  <dd>
                    abs {plan.nonlinear.absoluteTolerance.toExponential(1)} · rel{" "}
                    {plan.nonlinear.relativeTolerance.toExponential(1)}
                  </dd>
                </div>
                <div>
                  <dt>Safety bounds</dt>
                  <dd>
                    {plan.limits.maximumSteps.toLocaleString()} steps ·{" "}
                    {plan.nonlinear.maximumIterations} Newton iterations
                  </dd>
                </div>
                <div>
                  <dt>Plan key</dt>
                  <dd>
                    <code title={plan.key}>{plan.key.slice(-17)}</code>
                  </dd>
                </div>
              </dl>
            </details>
          </>
        )}
      </div>
      {active ? (
        <div className="run-progress" role="status" aria-live="polite">
          <div className="run-progress__line">
            <span>
              {status.kind === "cancelling" ? "Finishing accepted step…" : "Executing plan"}
            </span>
            <strong>{Math.round(completion * 100)}%</strong>
          </div>
          <progress aria-label="Accepted model-time progress" max={1} value={completion} />
          <p>
            {progress === null
              ? "Preparing the first accepted boundary."
              : `${progress.modelTime.toPrecision(4)} / ${progress.endTime.toPrecision(4)} s · ${progress.acceptedSteps.toLocaleString()} accepted steps`}
          </p>
        </div>
      ) : status.kind === "cancelled" ? (
        <p className="run-terminal-note" role="status">
          Cancelled at {status.cancellation.progress.modelTime.toPrecision(4)} s after{" "}
          {status.cancellation.progress.acceptedSteps.toLocaleString()} accepted steps. No partial
          result was admitted.
        </p>
      ) : null}
      <div className="run-actions">
        <button
          className="primary-action run-submit"
          disabled={!canRun}
          onClick={onRun}
          type="button"
        >
          <Icon name="play" />
          Run accepted plan
        </button>
        {active ? (
          <button
            className="secondary-action run-cancel"
            disabled={status.kind === "cancelling"}
            onClick={onCancel}
            type="button"
          >
            {status.kind === "cancelling" ? "Cancellation requested" : "Cancel at safe point"}
          </button>
        ) : null}
      </div>
    </section>
  );
}
