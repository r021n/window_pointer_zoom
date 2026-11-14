import { PanningPreview } from "./PanningPreview";
import {
  type ConfigConstraints,
  type PanningConfig,
  type PanningMode,
} from "../types/settings";
import { alphaToSpeed, describeSpeed, speedToAlpha } from "../utils/speed";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Slider } from "./ui/slider";

export interface PanningBehaviorPanelProps {
  config: PanningConfig;
  constraints: ConfigConstraints;
  onPreviewConfig: (next: PanningConfig) => void;
  onCommitConfig: (next?: PanningConfig) => void;
}


export function PanningBehaviorPanel({
  config,
  constraints,
  onPreviewConfig,
  onCommitConfig,
}: PanningBehaviorPanelProps) {
  const zoomSpeed = alphaToSpeed(config.zoomAlpha, constraints.alpha);
  const panSpeed = alphaToSpeed(config.panAlpha, constraints.alpha);
  const deadzoneDisabled = config.mode !== "edge_panning";

  const handleModeSelect = (mode: PanningMode) => {
    if (config.mode === mode) {
      return;
    }
    onCommitConfig({ ...config, mode });
  };

  const handleNumericSlider = (
    field: "edgeDeadzone",
    values: number[],
    commit: boolean,
  ) => {
    const val = values[0];
    if (typeof val !== "number" || !Number.isFinite(val)) {
      return;
    }
    const next: PanningConfig = {
      ...config,
      [field]: Number(val.toFixed(2)),
    };
    if (commit) {
      onCommitConfig(next);
    } else {
      onPreviewConfig(next);
    }
  };

  const handleSpeedSlider = (
    field: "zoomAlpha" | "panAlpha",
    values: number[],
    commit: boolean,
  ) => {
    const speed = values[0];
    if (typeof speed !== "number" || !Number.isFinite(speed)) {
      return;
    }
    const alpha = speedToAlpha(speed, constraints.alpha);
    const next: PanningConfig = { ...config, [field]: alpha };
    if (commit) {
      onCommitConfig(next);
    } else {
      onPreviewConfig(next);
    }
  };

  return (
    <div className="space-y-3">
      <Card aria-labelledby="panning-mode-heading">
        <CardHeader>
          <CardTitle id="panning-mode-heading">Panning Mode &amp; Deadzone</CardTitle>
        </CardHeader>
        <CardContent className="space-y-3">
          <PanningPreview mode={config.mode} />
          <div
            className="grid grid-cols-2 gap-2"
            role="radiogroup"
            aria-label="Panning Mode"
            onKeyDown={(event) => {
              if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) return;
              event.preventDefault();
              const mode = event.key === "Home" ? "center_follow" : event.key === "End" ? "edge_panning" : config.mode === "center_follow" ? "edge_panning" : "center_follow";
              handleModeSelect(mode);
              event.currentTarget.querySelector<HTMLButtonElement>(`[data-mode="${mode}"]`)?.focus();
            }}
          >
            <Button
              size="sm"
              role="radio"
              data-mode="center_follow"
              tabIndex={config.mode === "center_follow" ? 0 : -1}
              aria-checked={config.mode === "center_follow"}
              variant={config.mode === "center_follow" ? "default" : "outline"}
              onClick={() => handleModeSelect("center_follow")}
            >
              Center-locked
            </Button>
            <Button
              size="sm"
              role="radio"
              data-mode="edge_panning"
              tabIndex={config.mode === "edge_panning" ? 0 : -1}
              aria-checked={config.mode === "edge_panning"}
              variant={config.mode === "edge_panning" ? "default" : "outline"}
              onClick={() => handleModeSelect("edge_panning")}
            >
              Edge Panning
            </Button>
          </div>

          <p className="text-xs text-muted-foreground">
            {config.mode === "center_follow"
              ? "Pointer remains centered inside the magnified viewport."
              : "Viewport pans smoothly only when the cursor enters the outer deadzone margin."}
          </p>

          <div className="space-y-1">
            <div className="flex items-center justify-between gap-2">
              <span
                id="edge-deadzone-label"
                className="text-xs font-medium text-foreground"
              >
                Edge Deadzone
              </span>
              <span className="text-xs font-semibold tabular-nums text-primary">
                {Math.round(config.edgeDeadzone * 100)}%
              </span>
            </div>
            <Slider
              aria-labelledby="edge-deadzone-label"
              disabled={deadzoneDisabled}
              min={constraints.edgeDeadzone.min}
              max={constraints.edgeDeadzone.max}
              step={constraints.edgeDeadzone.step}
              value={[config.edgeDeadzone]}
              onValueChange={(values) =>
                handleNumericSlider("edgeDeadzone", values, false)
              }
              onValueCommit={(values) =>
                handleNumericSlider("edgeDeadzone", values, true)
              }
            />
          </div>
        </CardContent>
      </Card>

      <Card aria-labelledby="animation-speed-heading">
        <CardHeader>
          <CardTitle id="animation-speed-heading">Animation Speed</CardTitle>
          <span className="text-xs text-muted-foreground">
            Cinematic ↔ Instant
          </span>
        </CardHeader>
        <CardContent className="space-y-3">
          <div className="space-y-1">
            <div className="flex items-center justify-between gap-2">
              <span
                id="zoom-speed-label"
                className="text-xs font-medium text-foreground"
              >
                Zoom Speed
              </span>
              <span className="text-xs font-medium text-muted-foreground tabular-nums">
                {describeSpeed(zoomSpeed)} ({zoomSpeed}%)
              </span>
            </div>
            <Slider
              aria-labelledby="zoom-speed-label"
              min={0}
              max={100}
              step={1}
              value={[zoomSpeed]}
              onValueChange={(values) =>
                handleSpeedSlider("zoomAlpha", values, false)
              }
              onValueCommit={(values) =>
                handleSpeedSlider("zoomAlpha", values, true)
              }
            />
          </div>

          <div className="space-y-1">
            <div className="flex items-center justify-between gap-2">
              <span
                id="pan-speed-label"
                className="text-xs font-medium text-foreground"
              >
                Panning Speed
              </span>
              <span className="text-xs font-medium text-muted-foreground tabular-nums">
                {describeSpeed(panSpeed)} ({panSpeed}%)
              </span>
            </div>
            <Slider
              aria-labelledby="pan-speed-label"
              min={0}
              max={100}
              step={1}
              value={[panSpeed]}
              onValueChange={(values) =>
                handleSpeedSlider("panAlpha", values, false)
              }
              onValueCommit={(values) =>
                handleSpeedSlider("panAlpha", values, true)
              }
            />
          </div>
        </CardContent>
      </Card>
    </div>
  );
}
