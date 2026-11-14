import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  EngineSettingsSnapshot,
  PanningConfig,
  ShortcutConfig,
  ZoomLimits,
  ZoomStatus,
} from "../types/settings";

export function getZoomStatus(): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("get_zoom_status");
}

export function zoomIn(): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("zoom_in");
}

export function zoomOut(): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("zoom_out");
}

export function resetZoom(): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("reset_zoom");
}

export function setZoomLevel(scale: number): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("set_zoom_level", { scale });
}

export function updatePanningConfig(config: PanningConfig): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("update_panning_config", { config });
}

export function getEngineSettings(): Promise<EngineSettingsSnapshot> {
  return invoke<EngineSettingsSnapshot>("get_engine_settings");
}

export function updateZoomLimits(limits: ZoomLimits): Promise<ZoomStatus> {
  return invoke<ZoomStatus>("update_zoom_limits", { limits });
}

export function updateShortcuts(config: ShortcutConfig): Promise<ShortcutConfig> {
  return invoke<ShortcutConfig>("update_shortcuts", { config });
}

export function setShortcutsPaused(paused: boolean): Promise<void> {
  return invoke<void>("set_shortcuts_paused", { paused });
}

export function subscribeZoomStatus(
  onStatus: (status: ZoomStatus) => void,
): Promise<UnlistenFn> {
  return listen<ZoomStatus>("zoom-status-changed", (event) => {
    onStatus(event.payload);
  });
}
