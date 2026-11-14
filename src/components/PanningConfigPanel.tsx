import { useState, type ChangeEvent } from "react";
import type { PanningConfig, PanningMode } from "../services/tauri-commands";

export interface PanningConfigPanelProps {
  config: PanningConfig;
  onUpdateConfig: (config: PanningConfig) => void;
}

type NumericConfigKey = "zoomStep" | "zoomAlpha" | "panAlpha" | "edgeDeadzone";

interface NumericFieldSpec {
  key: NumericConfigKey;
  label: string;
  hint: string;
  min: number;
  max: number;
  step: number;
  requiresEdgePanning?: boolean;
}

const NUMERIC_FIELDS: readonly NumericFieldSpec[] = [
  {
    key: "zoomStep",
    label: "Zoom Step",
    hint: "Scale delta per zoom in/out (0.05 – 2.00)",
    min: 0.05,
    max: 2.0,
    step: 0.05,
  },
  {
    key: "zoomAlpha",
    label: "Zoom Alpha",
    hint: "Scale lerp responsiveness (0.01 – 1.00)",
    min: 0.01,
    max: 1.0,
    step: 0.01,
  },
  {
    key: "panAlpha",
    label: "Pan Alpha",
    hint: "Viewport pan responsiveness (0.01 – 1.00)",
    min: 0.01,
    max: 1.0,
    step: 0.01,
  },
  {
    key: "edgeDeadzone",
    label: "Edge Deadzone",
    hint: "Inner margin ratio for edge panning (0.05 – 0.45)",
    min: 0.05,
    max: 0.45,
    step: 0.01,
    requiresEdgePanning: true,
  },
];

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function PanningConfigPanel({
  config,
  onUpdateConfig,
}: PanningConfigPanelProps) {
  const [drafts, setDrafts] = useState<Partial<Record<NumericConfigKey, string>>>({});

  const handleModeChange = (event: ChangeEvent<HTMLSelectElement>) => {
    const mode = event.target.value as PanningMode;
    onUpdateConfig({ ...config, mode });
  };

  const handleNumericChange = (spec: NumericFieldSpec, raw: string) => {
    setDrafts((prev) => ({ ...prev, [spec.key]: raw }));

    if (raw.trim() === "") {
      return;
    }

    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) {
      return;
    }

    const clamped = clamp(parsed, spec.min, spec.max);
    onUpdateConfig({ ...config, [spec.key]: clamped });
  };

  const handleNumericBlur = (key: NumericConfigKey) => {
    setDrafts((prev) => {
      if (!(key in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[key];
      return next;
    });
  };

  return (
    <section className="panel" aria-labelledby="panning-config-heading">
      <div className="panel-header">
        <h2 id="panning-config-heading">Panning &amp; Smoothing</h2>
      </div>

      <div className="field-group">
        <label htmlFor="panning-mode-select">Panning Mode</label>
        <select
          id="panning-mode-select"
          value={config.mode}
          onChange={handleModeChange}
        >
          <option value="center_follow">Center Follow</option>
          <option value="edge_panning">Edge Panning</option>
        </select>
      </div>

      <div className="config-grid">
        {NUMERIC_FIELDS.map((field) => {
          const disabled =
            Boolean(field.requiresEdgePanning) && config.mode !== "edge_panning";
          const inputId = `config-${field.key}`;
          const value = drafts[field.key] ?? String(config[field.key]);

          return (
            <div className="field-group" key={field.key}>
              <label htmlFor={inputId}>{field.label}</label>
              <input
                id={inputId}
                type="number"
                min={field.min}
                max={field.max}
                step={field.step}
                value={value}
                disabled={disabled}
                onChange={(event) => handleNumericChange(field, event.target.value)}
                onBlur={() => handleNumericBlur(field.key)}
              />
              <span className="field-hint">{field.hint}</span>
            </div>
          );
        })}
      </div>
    </section>
  );
}
