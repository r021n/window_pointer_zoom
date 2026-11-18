export const UNMAGNIFIED_SCALE = 1.0;
export const SCALE_EPSILON = 0.001;

export type PanningMode = "center_follow" | "edge_panning";

export interface ZoomLimits {
  minScale: number;
  defaultScale: number;
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
  inputMapping: InputMappingStatus;
  active: boolean;
  targetScale: number;
  currentScale: number;
  config: PanningConfig;
  limits: ZoomLimits;
}

export interface InputMappingStatus {
  state: "pending" | "active" | "denied" | "error" | "disabled";
  uiAccess: boolean | null;
  errorCode: number | null;
  uiAccessErrorCode: number | null;
}

export interface ShortcutConfig {
  toggle: string;
  zoomIn: string;
  zoomOut: string;
  reset: string;
  pointerSizeUp: string | null;
  pointerSizeDown: string | null;
  pointerSizeReset: string | null;
}

export interface PointerSettings {
  enabled: boolean;
  defaultSize: number;
  step: number;
  clickEffectEnabled: boolean;
}

export interface PointerSettingsSnapshot {
  revision: number;
  settings: PointerSettings;
  currentSize: number | null;
  baselineSize: number | null;
  constraints: {
    minSize: number;
    maxSize: number;
    minStep: number;
    maxStep: number;
    defaultSize: number;
    defaultStep: number;
  };
  error: string | null;
  clickEffectAvailable: boolean;
}

export interface RangeBounds {
  min: number;
  max: number;
  step: number;
}

export interface ConfigConstraints {
  scaleEnvelope: RangeBounds;
  zoomStep: RangeBounds;
  alpha: RangeBounds;
  edgeDeadzone: RangeBounds;
  defaultLimits: ZoomLimits;
  defaultPanning: PanningConfig;
  defaultShortcuts: ShortcutConfig;
}

export interface EngineSettingsSnapshot {
  status: ZoomStatus;
  shortcuts: ShortcutConfig;
  constraints: ConfigConstraints;
}

export interface PersistedEngineSettings {
  limits: ZoomLimits;
  panning: PanningConfig;
}

export type ThemeMode = "light" | "dark" | "system";
