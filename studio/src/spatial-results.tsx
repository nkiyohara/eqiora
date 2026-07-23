import type { SpatialRunResult } from "./protocol";

const MESH_CELLS = [
  "r1c1",
  "r1c2",
  "r1c3",
  "r1c4",
  "r2c1",
  "r2c2",
  "r2c3",
  "r2c4",
  "r3c1",
  "r3c2",
  "r3c3",
  "r3c4",
  "r4c1",
  "r4c2",
  "r4c3",
  "r4c4",
] as const;

function elapsedLabel(seconds: number): string {
  if (seconds < 0.001) return `${Math.round(seconds * 1_000_000)} µs`;
  if (seconds < 1) return `${(seconds * 1_000).toFixed(1)} ms`;
  return `${seconds.toFixed(2)} s`;
}

function scientific(value: number): string {
  return value === 0 ? "0" : value.toExponential(3);
}

function methodLabel(result: SpatialRunResult): string {
  return result.plan.discretization.method === "finite-element" ? "FEM · Q1" : "FVM · cell";
}

export function SpatialResults({
  result,
  stale,
  staleReason,
  onViewField,
}: {
  readonly result: SpatialRunResult | null;
  readonly stale: boolean;
  readonly staleReason: string;
  readonly onViewField: (() => void) | null;
}) {
  return (
    <section className="results spatial-results" aria-labelledby="spatial-results-heading">
      <div className="section-line">
        <div>
          <span className="eyebrow">Bounded result · independent acceptance</span>
          <h2 id="spatial-results-heading">Solution evidence</h2>
        </div>
        {result === null ? null : (
          <div className="results__status">
            {stale ? (
              <span className="state-pill state-pill--warm">Retained</span>
            ) : (
              <span className="state-pill state-pill--ready">Verified</span>
            )}
            <span>{elapsedLabel(result.elapsedSeconds)}</span>
            {onViewField === null ? null : (
              <button className="secondary-action" onClick={onViewField} type="button">
                View field
              </button>
            )}
          </div>
        )}
      </div>

      {result === null ? (
        <div className="empty-state spatial-empty-state">
          <span aria-hidden="true">∬</span>
          <p>Run the accepted Realization to inspect bounded field and verification evidence.</p>
        </div>
      ) : (
        <>
          {stale ? (
            <p className="result-provenance">
              {staleReason} This result remains evidence for plan{" "}
              <code>{result.plan.key.slice(0, 10)}</code>.
            </p>
          ) : null}
          <div className="spatial-result-workspace">
            <section className="solution-summary" aria-label="Solution field summary">
              <div className="solution-summary__identity">
                <div className="mesh-glyph" aria-hidden="true">
                  {MESH_CELLS.map((cell) => (
                    <i key={cell} />
                  ))}
                </div>
                <div>
                  <span>{methodLabel(result)}</span>
                  <strong>{result.field.valueCount.toLocaleString()} values</strong>
                  <small>
                    {result.field.location.replace("-", " ")} · summary-only control response
                  </small>
                </div>
              </div>
              <div className="field-envelope">
                <div>
                  <span>Minimum</span>
                  <strong>{result.field.minimum.toPrecision(6)}</strong>
                </div>
                <div className="field-envelope__rule" aria-hidden="true">
                  <i />
                </div>
                <div>
                  <span>Maximum</span>
                  <strong>{result.field.maximum.toPrecision(6)}</strong>
                </div>
              </div>
              <div className="acceptance-pair">
                <div>
                  <span>True residual</span>
                  <strong>{scientific(result.solve.trueResidualNorm)}</strong>
                  <small>target ≤ {scientific(result.solve.residualTarget)}</small>
                </div>
                <div>
                  <span>Continuous balance</span>
                  <strong>{scientific(result.balance.relativeImbalance)}</strong>
                  <small>boundary + integrated source</small>
                </div>
              </div>
            </section>

            <aside
              className="evidence-inspector spatial-evidence"
              aria-labelledby="spatial-evidence-heading"
              id="evidence-inspector"
              tabIndex={-1}
            >
              <div className="evidence-inspector__heading">
                <div>
                  <span className="eyebrow">Accepted result projection</span>
                  <h3 id="spatial-evidence-heading">Execution evidence</h3>
                </div>
                <span className="state-pill state-pill--ready">Bounded</span>
              </div>
              <dl className="evidence-grid spatial-evidence-grid">
                <div>
                  <dt>Assembly</dt>
                  <dd>
                    <span>{result.assembly.execution.adapter}</span>
                    <small>{result.assembly.packetCount.toLocaleString()} local packets</small>
                  </dd>
                </div>
                <div>
                  <dt>Solver</dt>
                  <dd>
                    <span>{result.solve.backend}</span>
                    <small>{result.solve.completedIterations} CG iterations</small>
                  </dd>
                </div>
                <div>
                  <dt>Producer</dt>
                  <dd>
                    <span>{result.solve.execution.adapter}</span>
                    <small>{result.plan.placement.workers} run-owned worker(s)</small>
                  </dd>
                </div>
                <div>
                  <dt>Verifier</dt>
                  <dd>
                    <span>{result.solve.verification.adapter}</span>
                    <small>independent true residual</small>
                  </dd>
                </div>
                <div>
                  <dt>Balance observation</dt>
                  <dd>
                    <span>{scientific(result.balance.boundaryTotal)}</span>
                    <small>boundary + {scientific(result.balance.integratedSource)} source</small>
                  </dd>
                </div>
                <div>
                  <dt>Lineage</dt>
                  <dd>
                    <span>Realization r{result.plan.realizationRevision}</span>
                    <small title={result.digest}>{result.digest.slice(0, 12)}</small>
                  </dd>
                </div>
              </dl>
            </aside>
          </div>
        </>
      )}
    </section>
  );
}
