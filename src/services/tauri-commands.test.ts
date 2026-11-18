import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { listen, type Event } from "@tauri-apps/api/event";
import type {
  EngineSettingsSnapshot,
  PanningConfig,
  ShortcutConfig,
  ZoomLimits,
  ZoomStatus,
} from "../types/settings";
import {
  getEngineSettings,
  getZoomStatus,
  resetZoom,
  setShortcutsPaused,
  setZoomLevel,
  subscribeZoomStatus,
  updatePanningConfig,
  updateShortcuts,
  updateZoomLimits,
  zoomIn,
  zoomOut,
} from "./tauri-commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

const SAMPLE_CONFIG: PanningConfig = {
  mode: "center_follow",
  zoomStep: 0.25,
  zoomAlpha: 0.15,
  panAlpha: 0.25,
  edgeDeadzone: 0.15,
};

const SAMPLE_LIMITS: ZoomLimits = {
  minScale: 1.0,
  defaultScale: 2.0,
  maxScale: 5.0,
};

const SAMPLE_STATUS: ZoomStatus = {
  active: false,
  targetScale: 1.0,
  currentScale: 1.0,
  inputMapping: { state: "disabled", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
  config: SAMPLE_CONFIG,
  limits: SAMPLE_LIMITS,
};

const SAMPLE_SHORTCUTS: ShortcutConfig = {
  toggle: "Ctrl+Alt+Z",
  zoomIn: "Ctrl+Alt+Up",
  zoomOut: "Ctrl+Alt+Down",
  reset: "Ctrl+Alt+0",
  pointerSizeUp: null,
  pointerSizeDown: null,
  pointerSizeReset: null,
};

const SAMPLE_SNAPSHOT: EngineSettingsSnapshot = {
  status: SAMPLE_STATUS,
  shortcuts: SAMPLE_SHORTCUTS,
  constraints: {
    scaleEnvelope: { min: 1.0, max: 10.0, step: 0.1 },
    zoomStep: { min: 0.05, max: 2.0, step: 0.05 },
    alpha: { min: 0.01, max: 1.0, step: 0.01 },
    edgeDeadzone: { min: 0.05, max: 0.45, step: 0.01 },
    defaultLimits: SAMPLE_LIMITS,
    defaultPanning: SAMPLE_CONFIG,
    defaultShortcuts: SAMPLE_SHORTCUTS,
  },
};

describe("tauri-commands", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(SAMPLE_STATUS);
    mockedListen.mockReset();
  });

  it("invokes get_zoom_status", async () => {
    const result = await getZoomStatus();
    expect(mockedInvoke).toHaveBeenCalledWith("get_zoom_status");
    expect(result).toEqual(SAMPLE_STATUS);
  });

  it("invokes zoom_in, zoom_out, reset_zoom, and set_zoom_level", async () => {
    await zoomIn();
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_in");

    await zoomOut();
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_out");

    await resetZoom();
    expect(mockedInvoke).toHaveBeenCalledWith("reset_zoom");

    await setZoomLevel(2.5);
    expect(mockedInvoke).toHaveBeenCalledWith("set_zoom_level", { scale: 2.5 });
  });

  it("invokes update_panning_config and update_zoom_limits", async () => {
    const nextConfig: PanningConfig = {
      ...SAMPLE_CONFIG,
      mode: "edge_panning",
      edgeDeadzone: 0.2,
    };
    await updatePanningConfig(nextConfig);
    expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
      config: nextConfig,
    });

    const nextLimits: ZoomLimits = {
      minScale: 1.2,
      defaultScale: 2.5,
      maxScale: 6.0,
    };
    await updateZoomLimits(nextLimits);
    expect(mockedInvoke).toHaveBeenCalledWith("update_zoom_limits", {
      limits: nextLimits,
    });
  });

  it("invokes get_engine_settings, update_shortcuts, and set_shortcuts_paused", async () => {
    mockedInvoke.mockResolvedValueOnce(SAMPLE_SNAPSHOT);
    const snapshot = await getEngineSettings();
    expect(mockedInvoke).toHaveBeenCalledWith("get_engine_settings");
    expect(snapshot).toEqual(SAMPLE_SNAPSHOT);

    const nextShortcuts: ShortcutConfig = {
      ...SAMPLE_SHORTCUTS,
      toggle: "Ctrl+Shift+Z",
    };
    mockedInvoke.mockResolvedValueOnce(nextShortcuts);
    const updated = await updateShortcuts(nextShortcuts);
    expect(mockedInvoke).toHaveBeenCalledWith("update_shortcuts", {
      config: nextShortcuts,
    });
    expect(updated).toEqual(nextShortcuts);

    mockedInvoke.mockResolvedValueOnce(undefined);
    await setShortcutsPaused(true);
    expect(mockedInvoke).toHaveBeenCalledWith("set_shortcuts_paused", {
      paused: true,
    });
  });

  it("subscribes to zoom-status-changed, forwards payload, and returns unlisten", async () => {
    const unlistenStub = vi.fn();
    let capturedCallback: ((event: Event<ZoomStatus>) => void) | undefined;

    mockedListen.mockImplementationOnce(async (_eventName, cb) => {
      capturedCallback = cb as (event: Event<ZoomStatus>) => void;
      return unlistenStub;
    });

    const onStatus = vi.fn();
    const unlisten = await subscribeZoomStatus(onStatus);

    expect(mockedListen).toHaveBeenCalledWith(
      "zoom-status-changed",
      expect.any(Function),
    );
    expect(unlisten).toBe(unlistenStub);

    const updatedStatus: ZoomStatus = {
      ...SAMPLE_STATUS,
      active: true,
      targetScale: 2.0,
      currentScale: 1.5,
    };

    capturedCallback?.({
      event: "zoom-status-changed",
      id: 1,
      payload: updatedStatus,
    });

    expect(onStatus).toHaveBeenCalledTimes(1);
    expect(onStatus).toHaveBeenCalledWith(updatedStatus);
  });
});
