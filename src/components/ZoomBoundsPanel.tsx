import { clamp, formatScale } from "../lib/utils";
import type { RangeBounds, ZoomLimits } from "../types/settings";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { SliderField } from "./SliderField";

export interface ZoomBoundsPanelProps {
  limits: ZoomLimits;
  scaleEnvelope: RangeBounds;
  onPreviewLimits: (next: ZoomLimits) => void;
  onCommitLimits: (next?: ZoomLimits) => void;
}

function normalizeBounds(
  candidate: ZoomLimits,
  changedField: keyof ZoomLimits,
  envelope: RangeBounds,
): ZoomLimits {
  const minBound = envelope.min;
  const maxBound = envelope.max;

  let minScale = clamp(candidate.minScale, minBound, maxBound);
  let defaultScale = clamp(candidate.defaultScale, minBound, maxBound);
  let maxScale = clamp(candidate.maxScale, minBound, maxBound);

  if (changedField === "minScale") {
    defaultScale = Math.max(defaultScale, minScale);
    maxScale = Math.max(maxScale, defaultScale);
  } else if (changedField === "maxScale") {
    defaultScale = Math.min(defaultScale, maxScale);
    minScale = Math.min(minScale, defaultScale);
  } else {
    defaultScale = clamp(defaultScale, minScale, maxScale);
  }

  return {
    minScale: Number(minScale.toFixed(2)),
    defaultScale: Number(defaultScale.toFixed(2)),
    maxScale: Number(maxScale.toFixed(2)),
  };
}

export function ZoomBoundsPanel({
  limits,
  scaleEnvelope,
  onPreviewLimits,
  onCommitLimits,
}: ZoomBoundsPanelProps) {
  const handleBoundChange = (
    field: keyof ZoomLimits,
    values: number[],
    commit: boolean,
  ) => {
    const raw = values[0];
    if (typeof raw !== "number" || !Number.isFinite(raw)) {
      return;
    }

    const next = normalizeBounds(
      { ...limits, [field]: raw },
      field,
      scaleEnvelope,
    );

    if (commit) {
      onCommitLimits(next);
    } else {
      onPreviewLimits(next);
    }
  };

  const rows: ReadonlyArray<{
    key: keyof ZoomLimits;
    label: string;
    value: number;
  }> = [
    { key: "minScale", label: "Minimum Zoom", value: limits.minScale },
    {
      key: "defaultScale",
      label: "Default Toggle Zoom",
      value: limits.defaultScale,
    },
    { key: "maxScale", label: "Maximum Zoom", value: limits.maxScale },
  ];

  return (
    <Card aria-labelledby="zoom-bounds-heading">
      <CardHeader>
        <CardTitle id="zoom-bounds-heading">Zoom Bounds</CardTitle>
        <span className="text-xs text-muted-foreground tabular-nums">
          Toggle: 1.00x ↔ {formatScale(limits.defaultScale)}
        </span>
      </CardHeader>

      <CardContent className="space-y-3">
        {rows.map((row) => (
          <SliderField key={row.key} label={row.label} displayValue={formatScale(row.value)}
            min={scaleEnvelope.min} max={scaleEnvelope.max} step={scaleEnvelope.step} value={[row.value]}
            onValueChange={(values) => handleBoundChange(row.key, values, false)}
            onValueCommit={(values) => handleBoundChange(row.key, values, true)} />
        ))}
      </CardContent>
    </Card>
  );
}
