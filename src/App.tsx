import { PanningConfigPanel } from "./components/PanningConfigPanel";
import { ZoomControls } from "./components/ZoomControls";
import { useZoomController } from "./hooks/useZoomController";

export function App() {
  const {
    status,
    error,
    isUpdating,
    zoomIn,
    zoomOut,
    resetZoom,
    setZoomLevel,
    updatePanningConfig,
    clearError,
  } = useZoomController();

  return (
    <main className="app-shell">
      <header className="app-header">
        <div>
          <h1>Window Pointer Zoom</h1>
          <p className="app-subtitle">
            Fullscreen Windows desktop magnification controller
          </p>
        </div>
      </header>

      {error && (
        <div className="error-banner" role="alert">
          <span>{error}</span>
          <button
            type="button"
            className="error-dismiss"
            onClick={clearError}
            aria-label="Dismiss error"
          >
            Dismiss
          </button>
        </div>
      )}

      {!status ? (
        <section className="panel loading-panel" aria-live="polite">
          <p>
            {error
              ? "Unable to connect to zoom engine."
              : "Connecting to fullscreen zoom engine…"}
          </p>
        </section>
      ) : (
        <div className="panels-stack">
          <ZoomControls
            status={status}
            isUpdating={isUpdating}
            onZoomIn={() => void zoomIn()}
            onZoomOut={() => void zoomOut()}
            onResetZoom={() => void resetZoom()}
            onSetZoomLevel={(scale) => void setZoomLevel(scale)}
          />
          <PanningConfigPanel
            config={status.config}
            onUpdateConfig={(config) => void updatePanningConfig(config)}
          />
        </div>
      )}
    </main>
  );
}

export default App;
