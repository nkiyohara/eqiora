export function DemoLoadState({
  detail,
  glyph,
  title,
}: Readonly<{ detail: string; glyph: string; title: string }>) {
  return (
    <section className="empty-state" aria-live="polite" role="status">
      <span aria-hidden="true">{glyph}</span>
      <h1>{title}</h1>
      <p>{detail}</p>
    </section>
  );
}

export function DemoFailureBanner({
  message,
  onRetry,
}: Readonly<{ message: string; onRetry: () => void }>) {
  return (
    <div className="demo-failure" role="alert">
      <span>{message}</span>
      <button className="secondary-action" onClick={onRetry} type="button">
        Retry native demo
      </button>
    </div>
  );
}
