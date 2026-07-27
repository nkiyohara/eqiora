import {
  acceptanceSummary,
  EVIDENCE_LINKAGE_UNAVAILABLE,
  type EvidenceState,
  evidenceState,
  evidenceStateExplanation,
  evidenceStateLabel,
  evidenceStateMark,
  markedQuantity,
} from "./provenance";
import type { RunEvidence, RunResult } from "./reference-run-protocol";

const MAX_CHART_POINTS = 1_200;
const MAX_TABLE_ROWS = 120;

type SeriesSample = Readonly<{ index: number; time: number; value: number }>;

export function boundedSeriesSamples(
  time: readonly number[],
  values: readonly number[],
  maximumPoints: number,
): readonly SeriesSample[] {
  const length = Math.min(time.length, values.length);
  if (length === 0 || maximumPoints <= 0) return [];
  if (length <= maximumPoints) {
    return Array.from({ length }, (_, index) => ({
      index,
      time: time[index] ?? 0,
      value: values[index] ?? 0,
    }));
  }
  if (maximumPoints === 1) {
    return [{ index: 0, time: time[0] ?? 0, value: values[0] ?? 0 }];
  }
  if (maximumPoints === 2) {
    const finalIndex = length - 1;
    return [
      { index: 0, time: time[0] ?? 0, value: values[0] ?? 0 },
      {
        index: finalIndex,
        time: time[finalIndex] ?? 0,
        value: values[finalIndex] ?? 0,
      },
    ];
  }

  const samples: SeriesSample[] = [{ index: 0, time: time[0] ?? 0, value: values[0] ?? 0 }];
  const interiorBudget = Math.max(0, maximumPoints - 2);
  const bucketCount = Math.floor(interiorBudget / 2);
  for (let bucket = 0; bucket < bucketCount; bucket += 1) {
    const start = 1 + Math.floor((bucket * (length - 2)) / bucketCount);
    const end = 1 + Math.floor(((bucket + 1) * (length - 2)) / bucketCount);
    if (start >= end) continue;
    let minimumIndex = start;
    let maximumIndex = start;
    for (let index = start + 1; index < end; index += 1) {
      if ((values[index] ?? 0) < (values[minimumIndex] ?? 0)) minimumIndex = index;
      if ((values[index] ?? 0) > (values[maximumIndex] ?? 0)) maximumIndex = index;
    }
    for (const index of [minimumIndex, maximumIndex].sort((left, right) => left - right)) {
      if (samples[samples.length - 1]?.index !== index) {
        samples.push({ index, time: time[index] ?? 0, value: values[index] ?? 0 });
      }
    }
  }
  const finalIndex = length - 1;
  samples.push({
    index: finalIndex,
    time: time[finalIndex] ?? 0,
    value: values[finalIndex] ?? 0,
  });
  return samples;
}

function seriesPath(time: readonly number[], values: readonly number[]): string {
  const samples = boundedSeriesSamples(time, values, MAX_CHART_POINTS);
  if (samples.length === 0) {
    return "";
  }
  const width = 600;
  const height = 180;
  let minTime = samples[0]?.time ?? 0;
  let maxTime = minTime;
  let minValue = samples[0]?.value ?? 0;
  let maxValue = minValue;
  for (const sample of samples) {
    minTime = Math.min(minTime, sample.time);
    maxTime = Math.max(maxTime, sample.time);
    minValue = Math.min(minValue, sample.value);
    maxValue = Math.max(maxValue, sample.value);
  }
  const xRange = Math.max(maxTime - minTime, Number.EPSILON);
  const yRange = Math.max(maxValue - minValue, Number.EPSILON);
  return samples
    .map((sample, index) => {
      const x = ((sample.time - minTime) / xRange) * width;
      const y = height - ((sample.value - minValue) / yRange) * height;
      return `${index === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function elapsedLabel(seconds: number): string {
  if (seconds < 0.001) return `${Math.round(seconds * 1_000_000)} µs`;
  if (seconds < 1) return `${(seconds * 1_000).toFixed(1)} ms`;
  return `${seconds.toFixed(2)} s`;
}

function EvidenceInspector({
  digest,
  evidence,
  stale,
}: {
  readonly digest: string;
  readonly evidence: RunEvidence;
  readonly stale: boolean;
}) {
  const { plan } = evidence;
  const state = evidenceState(evidence, stale);
  const acceptance = acceptanceSummary(evidence);
  return (
    <aside
      className="evidence-inspector"
      aria-labelledby="evidence-heading"
      id="evidence-inspector"
      tabIndex={-1}
    >
      <div className="evidence-inspector__heading">
        <div>
          <span className="eyebrow">Immutable run record</span>
          <h3 id="evidence-heading">Evidence</h3>
        </div>
        <span className={`state-pill state-pill--${state}`} title={evidenceStateExplanation(state)}>
          <span aria-hidden="true">{evidenceStateMark(state)}</span> {evidenceStateLabel(state)}
        </span>
      </div>
      {/* A disclosure rather than a paragraph or a tooltip. `<summary>` is
          natively keyboard-focusable, so the explanation RFC 0076 requires is
          reachable without a pointer, and it gives this scrollable panel the
          focusable content WCAG 2.2 asks of one. */}
      <details className="evidence-inspector__state">
        <summary>What supports this result?</summary>
        <p>{evidenceStateExplanation(state)}</p>
      </details>
      <dl className="evidence-grid">
        <div>
          <dt>Producer</dt>
          <dd>
            <span>{plan.adapter.id}</span>
            <small>v{plan.adapter.version}</small>
          </dd>
        </div>
        <div>
          <dt>Placement</dt>
          <dd>
            <span>Host · {plan.placement.workers} worker</span>
            <small>Serial, run-local</small>
          </dd>
        </div>
        <div>
          <dt>Numerics</dt>
          <dd>
            <span>Backward Euler</span>
            <small>Dense finite-difference Newton</small>
          </dd>
        </div>
        <div>
          <dt>Acceptance</dt>
          <dd>
            <span>{acceptance.kind}</span>
            <small>{acceptance.verifier}</small>
          </dd>
        </div>
        <div>
          <dt>Output</dt>
          <dd>
            <span>
              {evidence.fieldCount} field · {evidence.sampleCount.toLocaleString()} samples
            </span>
            <small>{elapsedLabel(evidence.elapsedSeconds)} observed wall time</small>
          </dd>
        </div>
        <div>
          <dt>Registered evidence</dt>
          <dd>
            <span>Unavailable</span>
            <small>{EVIDENCE_LINKAGE_UNAVAILABLE}</small>
          </dd>
        </div>
        <div>
          <dt>Lineage</dt>
          <dd>
            <span>
              <code title={digest}>{digest.slice(0, 12)}</code>
            </span>
            <small title={plan.key}>{plan.key.slice(-17)}</small>
          </dd>
        </div>
      </dl>
    </aside>
  );
}

export function Results({
  configurationStale,
  revisionStale,
  result,
  sourceStale,
}: {
  readonly configurationStale: boolean;
  readonly revisionStale: boolean;
  readonly result: RunResult | null;
  readonly sourceStale: boolean;
}) {
  const series = result?.series[0] ?? null;
  const finalIndex = series === null ? -1 : series.values.length - 1;
  const stale = sourceStale || configurationStale || revisionStale;
  const staleLabel = revisionStale
    ? "Previous revision"
    : configurationStale
      ? "Previous run"
      : "Pending source";
  const staleReason = [
    revisionStale ? "The canonical revision has changed." : null,
    sourceStale ? "Source has uncompiled changes." : null,
    configurationStale ? "Run inputs have changed." : null,
  ]
    .filter((reason): reason is string => reason !== null)
    .join(" ");
  const tableSamples =
    series === null ? [] : boundedSeriesSamples(series.time, series.values, MAX_TABLE_ROWS);
  // Derived here as well as in the inspector, because RFC 0076 requires the
  // marking to travel with the value rather than live only in a side panel a
  // user has to know to open.
  const resultState: EvidenceState =
    result === null ? "stale" : evidenceState(result.evidence, stale);
  return (
    <section className="results" aria-labelledby="results-heading" aria-live="polite">
      <div className="section-line">
        <div>
          <span className="eyebrow">Owned result data</span>
          <h2 id="results-heading">Trajectory</h2>
        </div>
        {series === null ? null : (
          <div className="results__status">
            {stale ? <span className="state-pill state-pill--warm">{staleLabel}</span> : null}
            <span>{series.time.length} samples</span>
          </div>
        )}
      </div>
      {series === null || result === null ? (
        <div className="empty-state">
          <span aria-hidden="true">∿</span>
          <p>Run the validated revision to inspect its first field trajectory.</p>
        </div>
      ) : (
        <>
          {stale ? (
            <p className="result-provenance">
              {staleReason} This trajectory remains evidence for digest{" "}
              <code>{result?.digest.slice(0, 12)}</code>.
            </p>
          ) : null}
          <div className="result-workspace">
            <div className={stale ? "chart-wrap chart-wrap--stale" : "chart-wrap"}>
              <div className="chart-summary">
                <div className="chart-summary__item">
                  <span>Field</span>
                  <strong>{series.name}</strong>
                </div>
                <div className="chart-summary__item">
                  {/* Inline rather than on its own line, so marking the value
                      costs no vertical space. */}
                  <span>
                    Final value · <span aria-hidden="true">{evidenceStateMark(resultState)}</span>{" "}
                    {evidenceStateLabel(resultState)}
                  </span>
                  <strong
                    title={markedQuantity(
                      series.values[finalIndex] ?? 0,
                      series.dimension,
                      resultState,
                    )}
                  >
                    {(series.values[finalIndex] ?? 0).toPrecision(6)}
                  </strong>
                </div>
                <div className="chart-summary__item">
                  <span>Dimension</span>
                  <strong>[{series.dimension}]</strong>
                </div>
              </div>
              <svg className="trajectory-chart" role="img" viewBox="-8 -8 616 196">
                <title>
                  {series.name} over model time
                  {series.time.length > MAX_CHART_POINTS
                    ? `, extrema-preserving preview of ${series.time.length} samples`
                    : ""}
                </title>
                <path className="trajectory-grid" d="M0 45H600M0 90H600M0 135H600" />
                <path className="trajectory-line" d={seriesPath(series.time, series.values)} />
              </svg>
              <div className="sr-only">
                <table>
                  <caption>
                    {series.name} bounded trajectory sample table in [{series.dimension}],{" "}
                    {tableSamples.length} of {series.time.length} rows.{" "}
                    {evidenceStateLabel(resultState)}. {evidenceStateExplanation(resultState)}
                  </caption>
                  <thead>
                    <tr>
                      <th>Sample</th>
                      <th>Time</th>
                      <th>Value</th>
                    </tr>
                  </thead>
                  <tbody>
                    {tableSamples.map((sample) => (
                      <tr key={sample.index}>
                        <td>{sample.index}</td>
                        <td>{sample.time}</td>
                        {/* Marked, not bare: this table is the copy and
                            screen-reader path, and a value detached from its
                            state reads as carrying more support than it does. */}
                        <td>{markedQuantity(sample.value, series.dimension, resultState)}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
            <EvidenceInspector digest={result.digest} evidence={result.evidence} stale={stale} />
          </div>
        </>
      )}
    </section>
  );
}
