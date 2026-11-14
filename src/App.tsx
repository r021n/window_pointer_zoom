import { Sliders, Wrench, ZoomIn } from "lucide-react";
import { PanningBehaviorPanel } from "./components/PanningBehaviorPanel";
import { ShortcutConfigurator } from "./components/ShortcutConfigurator";
import { SystemSettingsPanel } from "./components/SystemSettingsPanel";
import { ZoomBoundsPanel } from "./components/ZoomBoundsPanel";
import { ZoomStepPanel } from "./components/ZoomStepPanel";
import { ZoomControls } from "./components/ZoomControls";
import { Button } from "./components/ui/button";
import { Card, CardContent } from "./components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./components/ui/tabs";
import { useAppSettings } from "./hooks/useAppSettings";
import { useTheme } from "./hooks/useTheme";
import { useZoomController } from "./hooks/useZoomController";

export function App() {
  const { theme, setTheme } = useTheme();

  const {
    status,
    error: zoomError,
    isUpdating,
    zoomIn,
    zoomOut,
    resetZoom,
    setZoomLevel,
    syncStatus,
    clearError: clearZoomError,
  } = useZoomController();

  const {
    limits,
    panning,
    shortcuts,
    constraints,
    autostartEnabled,
    isLoading: isSettingsLoading,
    isSaving,
    error: settingsError,
    shortcutError,
    previewZoomLimits,
    commitZoomLimits,
    previewPanningConfig,
    commitPanningConfig,
    updateShortcuts,
    setShortcutsPaused,
    clearShortcutError,
    toggleAutostart,
    resetToDefaults,
    clearError: clearSettingsError,
  } = useAppSettings({ onStatusSynced: syncStatus });

  const combinedError = zoomError ?? settingsError;

  const handleDismissError = () => {
    clearZoomError();
    clearSettingsError();
  };

  const isReady = Boolean(
    status && limits && panning && shortcuts && constraints && !isSettingsLoading,
  );

  return (
    <main className="mx-auto flex h-screen max-w-[640px] flex-col overflow-hidden bg-background px-6 py-5 text-foreground">
      <header className="mb-5 flex shrink-0 items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl border border-primary/20 bg-accent text-primary"><ZoomIn className="h-5 w-5" /></div>
        <div>
          <p className="mb-0.5 text-[10px] font-semibold uppercase tracking-[0.2em] text-muted-foreground">Desktop magnification</p>
          <h1 className="text-lg font-semibold tracking-tight">Window Pointer Zoom</h1>
        </div>
      </header>

      {combinedError && (
        <div
          role="alert"
          className="mb-3 flex shrink-0 items-center justify-between gap-2 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs text-destructive"
        >
          <span className="min-w-0 break-words">{combinedError}</span>
          <Button
            size="sm"
            variant="ghost"
            className="h-6 shrink-0 px-2 text-destructive hover:bg-destructive/15"
            onClick={handleDismissError}
            aria-label="Dismiss error"
          >
            Dismiss
          </Button>
        </div>
      )}

      {!isReady ? (
        <Card aria-live="polite">
          <CardContent className="py-6 text-center text-xs text-muted-foreground">
            {combinedError
              ? "Unable to connect to zoom engine."
              : "Connecting to fullscreen zoom engine…"}
          </CardContent>
        </Card>
      ) : (
        <Tabs defaultValue="controls" className="flex min-h-0 flex-1 flex-col">
          <TabsList className="mb-3 shrink-0">
            <TabsTrigger value="controls">
              <ZoomIn className="h-3.5 w-3.5" />
              Zoom
            </TabsTrigger>
            <TabsTrigger value="behavior">
              <Sliders className="h-3.5 w-3.5" />
              Motion
            </TabsTrigger>
            <TabsTrigger value="system">
              <Wrench className="h-3.5 w-3.5" />
              Settings
            </TabsTrigger>
          </TabsList>

          <div className="min-h-0 flex-1 overflow-y-auto pr-0.5">
            <TabsContent value="controls">
              <ZoomControls
                status={status!}
                isUpdating={isUpdating}
                onZoomIn={() => void zoomIn()}
                onZoomOut={() => void zoomOut()}
                onResetZoom={() => void resetZoom()}
                onSetZoomLevel={setZoomLevel}
              />
              <ZoomStepPanel
                config={panning!}
                constraints={constraints!}
                onPreviewConfig={previewPanningConfig}
                onCommitConfig={(next) => void commitPanningConfig(next)}
              />
              <ZoomBoundsPanel
                limits={limits!}
                scaleEnvelope={constraints!.scaleEnvelope}
                onPreviewLimits={previewZoomLimits}
                onCommitLimits={(next) => void commitZoomLimits(next)}
              />
            </TabsContent>

            <TabsContent value="behavior">
              <PanningBehaviorPanel
                config={panning!}
                constraints={constraints!}
                onPreviewConfig={previewPanningConfig}
                onCommitConfig={(next) => void commitPanningConfig(next)}
              />
            </TabsContent>

            <TabsContent value="system">
              <ShortcutConfigurator
                shortcuts={shortcuts!}
                shortcutError={shortcutError}
                isSaving={isSaving}
                onUpdateShortcuts={updateShortcuts}
                onSetShortcutsPaused={setShortcutsPaused}
                onClearShortcutError={clearShortcutError}
              />
              <SystemSettingsPanel
                autostartEnabled={autostartEnabled}
                theme={theme}
                isSaving={isSaving}
                onToggleAutostart={(enabled) => void toggleAutostart(enabled)}
                onChangeTheme={(next) => void setTheme(next)}
                onResetToDefaults={() => void resetToDefaults()}
              />
            </TabsContent>
          </div>
        </Tabs>
      )}
    </main>
  );
}

export default App;
