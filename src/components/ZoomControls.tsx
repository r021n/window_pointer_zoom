import { Minus, Plus, RotateCcw } from "lucide-react";
import { clamp, formatScale } from "../lib/utils";
import {
  SCALE_EPSILON,
  UNMAGNIFIED_SCALE,
  type ZoomStatus,
} from "../types/settings";
import { Badge } from "./ui/badge";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Slider } from "./ui/slider";

export interface ZoomControlsProps {
  status: ZoomStatus;
  isUpdating: boolean;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onResetZoom: () => void;
  onSetZoomLevel: (scale: number) => void;
}

const SLIDER_STEP = 0.05;

export function ZoomControls({
  status,
  isUpdating,
  onZoomIn,
  onZoomOut,
  onResetZoom,
  onSetZoomLevel,
}: ZoomControlsProps) {
  const { active, currentScale, targetScale, limits } = status;
  const atMaxScale = targetScale >= limits.maxScale - SCALE_EPSILON;
  const atUnmagnifiedScale = targetScale <= UNMAGNIFIED_SCALE + SCALE_EPSILON;
  const sliderValue = clamp(targetScale, UNMAGNIFIED_SCALE, limits.maxScale);

  const handleSliderChange = (values: number[]) => {
    const nextScale = values[0];
    if (typeof nextScale === "number" && Number.isFinite(nextScale)) {
      onSetZoomLevel(nextScale);
    }
  };

  return (
    <Card className="rounded-2xl border border-border bg-card shadow-sm" aria-labelledby="zoom-controls-heading">
      <CardHeader>
        <CardTitle id="zoom-controls-heading">Zoom Controls</CardTitle>
        <Badge
          variant={active ? "success" : "secondary"}
          data-testid="zoom-status-badge"
        >
          {active ? "Active" : "Inactive"}
        </Badge>
      </CardHeader>

      <CardContent className="space-y-3">
        <div className="py-4 text-center">
          <span className="text-xs text-muted-foreground">Current magnification</span>
          <strong className="mt-1 block text-5xl font-light tracking-tight tabular-nums" data-testid="current-scale">{formatScale(currentScale)}</strong>
          <p className="mt-2 h-5 text-xs text-muted-foreground" aria-hidden={Math.abs(currentScale - targetScale) < SCALE_EPSILON}>
            {Math.abs(currentScale - targetScale) >= SCALE_EPSILON && <>Adjusting to <span data-testid="target-scale">{formatScale(targetScale)}</span></>}
          </p>
        </div>

        <div className="grid grid-cols-3 gap-2">
          <Button
            variant="default"
            onClick={onZoomIn}
            disabled={atMaxScale}
          >
            <Plus className="h-3.5 w-3.5" />
            Zoom In
          </Button>
          <Button
            variant="secondary"
            onClick={onZoomOut}
            disabled={atUnmagnifiedScale}
          >
            <Minus className="h-3.5 w-3.5" />
            Zoom Out
          </Button>
          <Button
            variant="outline"
            onClick={onResetZoom}
            disabled={!active && atUnmagnifiedScale}
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Reset ({formatScale(UNMAGNIFIED_SCALE)})
          </Button>
        </div>

        <div className="space-y-1.5">
          <div className="flex items-baseline justify-between gap-2">
            <span
              id="target-scale-slider-label"
              className="text-xs font-medium text-foreground"
            >
              Target Zoom Level
            </span>
            <span className="text-[11px] text-muted-foreground tabular-nums">
              Active range {formatScale(limits.minScale)} –{" "}
              {formatScale(limits.maxScale)}
              {isUpdating ? " · Updating…" : ""}
            </span>
          </div>
          <Slider
            aria-labelledby="target-scale-slider-label"
            min={UNMAGNIFIED_SCALE}
            max={limits.maxScale}
            step={SLIDER_STEP}
            value={[sliderValue]}
            onValueChange={handleSliderChange}
          />
        </div>
      </CardContent>
    </Card>
  );
}
