import { invoke } from "@tauri-apps/api/core";

export type PanningMode = "center_follow" | "edge_panning";

export interface ZoomLimits {
  minScale: number;
  maxScale: number;
}

export interface PanningConfig {
  mode: PanningMode;
  zoomStep: number;
  zoomAlpha: number;
  panAlpha: number;
  edgeDeadzone: number;
}

export interface ZoomStatus {
  active: boolean;
  targetScale: number;
  currentScale: number;
  config: PanningConfig;
  limits: ZoomLimits;
}

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
