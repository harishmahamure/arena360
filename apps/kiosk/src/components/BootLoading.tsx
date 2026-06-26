/** Boot loader styled by inline CSS in index.html (works when Vite CSS fails). */
export function BootLoading() {
  return (
    <div className="boot-loading" role="status" aria-live="polite">
      <div className="boot-loading-spinner" aria-hidden="true" />
      <p className="boot-loading-text">Loading station…</p>
    </div>
  );
}
