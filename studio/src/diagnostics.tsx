import type { StudioDiagnostic } from "./protocol";

export interface DiagnosticPresentation {
  readonly diagnostic: StudioDiagnostic;
  readonly location: string | null;
  readonly navigable: boolean;
}

export function Diagnostics({
  diagnostics,
  onNavigate,
}: {
  readonly diagnostics: readonly DiagnosticPresentation[];
  readonly onNavigate: (diagnostic: StudioDiagnostic) => void;
}) {
  return (
    <section className="diagnostics" aria-labelledby="diagnostics-heading" aria-live="polite">
      <div className="section-line">
        <h2 id="diagnostics-heading">Diagnostics</h2>
        <span>{diagnostics.length}</span>
      </div>
      {diagnostics.length === 0 ? (
        <div className="empty-state empty-state--success">
          <span aria-hidden="true">✓</span>
          <p>No diagnostics for the current operation.</p>
        </div>
      ) : (
        <ol className="diagnostic-list">
          {diagnostics.map(({ diagnostic, location, navigable }) => (
            <li
              key={`${diagnostic.code}:${diagnostic.message}:${diagnostic.graphPath === null ? "" : JSON.stringify(diagnostic.graphPath)}:${diagnostic.span?.start ?? ""}`}
              className={`diagnostic diagnostic--${diagnostic.severity}`}
            >
              {navigable ? (
                <button
                  aria-label={`Go to ${location ?? "diagnostic source"}: ${diagnostic.message}`}
                  className="diagnostic__action"
                  onClick={() => onNavigate(diagnostic)}
                  type="button"
                >
                  <DiagnosticContent diagnostic={diagnostic} location={location} />
                </button>
              ) : (
                <div className="diagnostic__content">
                  <DiagnosticContent diagnostic={diagnostic} location={location} />
                  {diagnostic.span === null || location === null ? null : (
                    <span className="diagnostic__stale">Source changed since this diagnostic.</span>
                  )}
                </div>
              )}
            </li>
          ))}
        </ol>
      )}
    </section>
  );
}

function DiagnosticContent({
  diagnostic,
  location,
}: {
  readonly diagnostic: StudioDiagnostic;
  readonly location: string | null;
}) {
  return (
    <>
      <span className="diagnostic__meta">
        <strong>{diagnostic.code}</strong>
        <span>{diagnostic.severity}</span>
      </span>
      <span className="diagnostic__message">{diagnostic.message}</span>
      {location === null ? null : <code>{location}</code>}
      {diagnostic.graphPath === null ? null : (
        <code>
          {Array.isArray(diagnostic.graphPath)
            ? diagnostic.graphPath.join(" › ")
            : diagnostic.graphPath}
        </code>
      )}
      {diagnostic.patch === null || diagnostic.patch === undefined ? null : (
        <span className="diagnostic__patch">{diagnostic.patch.summary}</span>
      )}
    </>
  );
}
