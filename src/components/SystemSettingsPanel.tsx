import { RotateCcw } from "lucide-react";
import type { ThemeMode } from "../types/settings";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";
import { Label } from "./ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "./ui/select";
import { Switch } from "./ui/switch";

export interface SystemSettingsPanelProps {
  autostartEnabled: boolean;
  theme: ThemeMode;
  isSaving: boolean;
  onToggleAutostart: (enabled: boolean) => void;
  onChangeTheme: (theme: ThemeMode) => void;
  onResetToDefaults: () => void;
}

export function SystemSettingsPanel({
  autostartEnabled,
  theme,
  isSaving,
  onToggleAutostart,
  onChangeTheme,
  onResetToDefaults,
}: SystemSettingsPanelProps) {
  return (
    <Card aria-labelledby="system-settings-heading">
      <CardHeader>
        <CardTitle id="system-settings-heading">System &amp; Appearance</CardTitle>
      </CardHeader>

      <CardContent className="space-y-3">
        <div className="flex items-center justify-between gap-3 border-b border-border py-3">
          <div className="space-y-0.5">
            <Label htmlFor="autostart-switch" className="text-xs font-medium">
              Start on Boot
            </Label>
            <p className="text-xs text-muted-foreground">
              Launch minimized to system tray on Windows sign-in
            </p>
          </div>
          <Switch
            id="autostart-switch"
            checked={autostartEnabled}
            onCheckedChange={onToggleAutostart}
          />
        </div>

        <div className="flex items-center justify-between gap-3 border-b border-border py-3">
          <Label htmlFor="theme-select" className="text-xs font-medium">
            Theme
          </Label>
          <div className="w-36">
            <Select
              value={theme}
              onValueChange={(value) => onChangeTheme(value as ThemeMode)}
            >
              <SelectTrigger id="theme-select" aria-label="Theme">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="system">System</SelectItem>
                <SelectItem value="light">Light</SelectItem>
                <SelectItem value="dark">Dark</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div className="flex items-center justify-between gap-3 pt-1">
          <p className="text-xs text-muted-foreground">
            Restores zoom bounds, panning behavior, and shortcuts.
          </p>
          <Button
            size="sm"
            variant="outline"
            disabled={isSaving}
            onClick={onResetToDefaults}
          >
            <RotateCcw className="h-3.5 w-3.5" />
            Reset to Defaults
          </Button>
        </div>
      </CardContent>
    </Card>
  );
}
