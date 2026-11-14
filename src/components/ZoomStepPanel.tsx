import { formatScale } from "../lib/utils";
import {
  SCALE_EPSILON,
  type ConfigConstraints,
  type PanningConfig,
} from "../types/settings";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Slider } from "./ui/slider";

export interface ZoomStepPanelProps {
  config: PanningConfig;
  constraints: ConfigConstraints;
  onPreviewConfig: (next: PanningConfig) => void;
  onCommitConfig: (next?: PanningConfig) => void;
}

const ZOOM_STEP_PRESETS = [0.1, 0.25, 0.5] as const;

export function ZoomStepPanel({
  config,
  constraints,
  onPreviewConfig,
  onCommitConfig,
}: ZoomStepPanelProps) {
  const handleStepPreset = (zoomStep: number) => {
    onCommitConfig({ ...config, zoomStep });
  };

  const handleNumericSlider = (
    values: number[],
    commit: boolean,
  ) => {
    const val = values[0];
    if (typeof val !== "number" || !Number.isFinite(val)) {
      return;
    }
    const next: PanningConfig = {
      ...config,
      zoomStep: Number(val.toFixed(2)),
    };
    if (commit) {
      onCommitConfig(next);
    } else {
      onPreviewConfig(next);
    }
  };

  return (
      <Card aria-labelledby="step-zoom-heading">
        <CardHeader>
          <CardTitle id="step-zoom-heading">Step Zoom</CardTitle>
          <span className="text-xs font-semibold tabular-nums text-primary">
            {formatScale(config.zoomStep)}
          </span>
        </CardHeader>
        <CardContent className="space-y-2.5">
          <div
            className="grid grid-cols-3 gap-2"
            role="group"
            aria-label="Zoom step presets"
          >
            {ZOOM_STEP_PRESETS.map((preset) => {
              const isSelected =
                Math.abs(config.zoomStep - preset) < SCALE_EPSILON;
              return (
                <Button
                  key={preset}
                  size="sm"
                  variant={isSelected ? "default" : "outline"}
                  aria-pressed={isSelected}
                  onClick={() => handleStepPreset(preset)}
                >
                  {preset}x
                </Button>
              );
            })}
          </div>

          <div className="space-y-1">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <span id="zoom-step-slider-label">Fine Step Increment</span>
              <span className="tabular-nums">
                {constraints.zoomStep.min.toFixed(2)}x –{" "}
                {constraints.zoomStep.max.toFixed(2)}x
              </span>
            </div>
            <Slider
              aria-labelledby="zoom-step-slider-label"
              min={constraints.zoomStep.min}
              max={constraints.zoomStep.max}
              step={constraints.zoomStep.step}
              value={[config.zoomStep]}
              onValueChange={(values) =>
                handleNumericSlider(values, false)
              }
              onValueCommit={(values) =>
                handleNumericSlider(values, true)
              }
            />
          </div>
        </CardContent>
      </Card>

  );
}
