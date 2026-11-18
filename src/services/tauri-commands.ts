import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  EngineSettingsSnapshot,
  PanningConfig,
  PointerSettings,
  PointerSettingsSnapshot,
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

export function getPointerSettings(): Promise<PointerSettingsSnapshot> {
  return invoke<PointerSettingsSnapshot>("get_pointer_settings");
}

export function updatePointerSettings(
  settings: PointerSettings,
): Promise<PointerSettingsSnapshot> {
  return invoke<PointerSettingsSnapshot>("update_pointer_settings", { settings });
}

export function applyDefaultPointerSize(): Promise<PointerSettingsSnapshot> {
  return invoke<PointerSettingsSnapshot>("apply_default_pointer_size");
}

export function resetPointerSize(): Promise<PointerSettingsSnapshot> {
  return invoke<PointerSettingsSnapshot>("reset_pointer_size");
}

export function subscribePointerStatus(
  onStatus: (status: PointerSettingsSnapshot) => void,
): Promise<UnlistenFn> {
  return listen<PointerSettingsSnapshot>("pointer-status-changed", (event) => {
    onStatus(event.payload);
  });
}

export function subscribeZoomStatus(
  onStatus: (status: ZoomStatus) => void,
): Promise<UnlistenFn> {
  return listen<ZoomStatus>("zoom-status-changed", (event) => {
    onStatus(event.payload);
  });
}
