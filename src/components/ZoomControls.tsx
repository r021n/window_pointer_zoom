import type { ChangeEvent } from "react";
import type { ZoomStatus } from "../services/tauri-commands";

export interface ZoomControlsProps {
  status: ZoomStatus;
  isUpdating: boolean;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onSetZoomLevel: (scale: number) => void;
}

const SLIDER_STEP = 0.05;
const SCALE_BOUND_EPSILON = 0.001;

function formatScale(scale: number): string {
  return `${scale.toFixed(2)}x`;
}

export function ZoomControls({
  status,
  isUpdating,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onSetZoomLevel,
}: ZoomControlsProps) {
  const { active, currentScale, targetScale, limits } = status;
  const atMaxScale = targetScale >= limits.maxScale - SCALE_BOUND_EPSILON;
  const atMinScale = targetScale <= limits.minScale + SCALE_BOUND_EPSILON;

  const handleSliderChange = (event: ChangeEvent<HTMLInputElement>) => {
    const nextScale = Number(event.target.value);
    if (Number.isFinite(nextScale)) {
      onSetZoomLevel(nextScale);
    }
  };

  return (
    <section className="panel" aria-labelledby="zoom-controls-heading">
      <div className="panel-header">
        <h2 id="zoom-controls-heading">Zoom Controls</h2>
        <span
          className={`status-badge ${active ? "status-badge--active" : "status-badge--idle"}`}
          data-testid="zoom-status-badge"
        >
          {active ? "Active" : "Inactive"}
        </span>
      </div>

      <div className="metrics-grid">
        <div className="metric-card">
          <span className="metric-label">Current Scale</span>
          <strong className="metric-value" data-testid="current-scale">
            {formatScale(currentScale)}
          </strong>
        </div>
        <div className="metric-card">
          <span className="metric-label">Target Scale</span>
          <strong className="metric-value" data-testid="target-scale">
            {formatScale(targetScale)}
          </strong>
        </div>
      </div>

      <div className="button-row">
        <button
          type="button"
          className="btn btn-primary"
          onClick={onZoomIn}
          disabled={atMaxScale}
        >
          Zoom In
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onZoomOut}
          disabled={atMinScale}
        >
          Zoom Out
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={onResetZoom}
          disabled={!active && atMinScale}
        >
          Reset ({formatScale(limits.minScale)})
        </button>
      </div>

      <div className="slider-group">
        <div className="slider-header">
          <label htmlFor="target-scale-slider">Target Zoom Level</label>
          <span className="slider-range-hint">
            {formatScale(limits.minScale)} – {formatScale(limits.maxScale)}
            {isUpdating ? " · Updating…" : ""}
          </span>
        </div>
        <input
          id="target-scale-slider"
          type="range"
          min={limits.minScale}
          max={limits.maxScale}
          step={SLIDER_STEP}
          value={targetScale}
          onChange={handleSliderChange}
        />
      </div>
    </section>
  );
}
