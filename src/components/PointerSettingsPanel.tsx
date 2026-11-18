import { MousePointer2 } from "lucide-react";
import type { PointerSettings, PointerSettingsSnapshot } from "../types/settings";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";

interface NumberFieldProps {
  name: string;
  label: string;
  value: number;
  min: number;
  max: number;
}

function NumberField({ name, label, value, min, max }: NumberFieldProps) {
  return (
    <label className="flex items-center justify-between gap-3 text-xs font-medium">
      {label}
      <input
        key={value}
        className="h-8 w-24 rounded-md border border-border bg-background px-2.5 text-right tabular-nums"
        type="number"
        name={name}
        min={min}
        max={max}
        step={1}
        defaultValue={value}
        required
      />
    </label>
  );
}

interface PointerSettingsPanelProps {
  snapshot: PointerSettingsSnapshot | null;
  error: string | null;
  isUpdating: boolean;
  onUpdateSettings: (settings: PointerSettings) => void;
  onApplyDefaultSize: () => void;
  onRestoreSize: () => void;
}

export function PointerSettingsPanel({
  snapshot,
  error,
  isUpdating,
  onUpdateSettings,
  onApplyDefaultSize,
  onRestoreSize,
}: PointerSettingsPanelProps) {
  return (
    <Card aria-labelledby="pointer-settings-heading">
      <CardHeader>
        <CardTitle id="pointer-settings-heading" className="flex items-center gap-1.5">
          <MousePointer2 className="h-4 w-4" />
          Pointer
        </CardTitle>
        {snapshot?.currentSize != null && (
          <span aria-live="polite" className="text-xs font-semibold tabular-nums text-primary">
            Current size {snapshot.currentSize} px
          </span>
        )}
      </CardHeader>
      <CardContent className="space-y-3">
        {error && <p role="alert" className="text-xs text-destructive">{error}</p>}
        {!snapshot && !error && (
          <p role="status" className="text-xs text-muted-foreground">Loading pointer settings…</p>
        )}
        {snapshot && (
          <>
            <div className="flex items-center justify-between gap-3 border-b border-border pb-3">
              <Label htmlFor="pointer-sizing-switch">Pointer size controls</Label>
              <Switch
                id="pointer-sizing-switch"
                checked={snapshot.settings.enabled}
                disabled={isUpdating}
                onCheckedChange={(enabled) => onUpdateSettings({ ...snapshot.settings, enabled })}
              />
            </div>
            <form
              onSubmit={(event) => {
                event.preventDefault();
                const values = new FormData(event.currentTarget);
                const defaultSize = Number(values.get("defaultSize"));
                const step = Number(values.get("step"));
                if (defaultSize !== snapshot.settings.defaultSize || step !== snapshot.settings.step) {
                  onUpdateSettings({ ...snapshot.settings, defaultSize, step });
                }
              }}
            >
              <fieldset disabled={isUpdating} className="space-y-3">
                <NumberField
                  name="defaultSize"
                  label="Default size (px)"
                  value={snapshot.settings.defaultSize}
                  min={snapshot.constraints.minSize}
                  max={snapshot.constraints.maxSize}
                />
                <NumberField
                  name="step"
                  label="Size step (px)"
                  value={snapshot.settings.step}
                  min={snapshot.constraints.minStep}
                  max={snapshot.constraints.maxStep}
                />
                <div className="flex justify-end">
                  <Button type="submit" size="sm" variant="outline" disabled={isUpdating}>
                    Save sizes
                  </Button>
                </div>
              </fieldset>
            </form>
            <div className="flex flex-wrap items-center justify-end gap-2 border-b border-border pb-3">
              {snapshot.baselineSize != null && (
                <span className="mr-auto text-xs text-muted-foreground">
                  Windows size {snapshot.baselineSize} px
                </span>
              )}
              <Button
                size="sm"
                variant="outline"
                disabled={isUpdating || !snapshot.settings.enabled}
                onClick={onApplyDefaultSize}
              >
                Apply default size
              </Button>
              <Button
                size="sm"
                variant="outline"
                disabled={isUpdating || !snapshot.settings.enabled}
                onClick={onRestoreSize}
              >
                Restore Windows size
              </Button>
            </div>
            <div className="flex items-center justify-between gap-3">
              <div>
                <Label htmlFor="click-effect-switch">Click effect</Label>
                {!snapshot.clickEffectAvailable && (
                  <p className="text-xs text-muted-foreground">Click effect is unavailable.</p>
                )}
              </div>
              <Switch
                id="click-effect-switch"
                checked={snapshot.settings.clickEffectEnabled}
                disabled={
                  isUpdating || (!snapshot.clickEffectAvailable && !snapshot.settings.clickEffectEnabled)
                }
                onCheckedChange={(clickEffectEnabled) =>
                  onUpdateSettings({ ...snapshot.settings, clickEffectEnabled })
                }
              />
            </div>
          </>
        )}
      </CardContent>
    </Card>
  );
}
