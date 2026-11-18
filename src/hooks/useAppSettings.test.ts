import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as autostartService from "../services/autostart";
import * as settingsStore from "../services/settings-store";
import * as tauriCommands from "../services/tauri-commands";
import type {
  EngineSettingsSnapshot,
  PanningConfig,
  ShortcutConfig,
  ZoomLimits,
  ZoomStatus,
} from "../types/settings";
import { useAppSettings } from "./useAppSettings";

vi.mock("../services/tauri-commands", () => ({
  getEngineSettings: vi.fn(),
  updateZoomLimits: vi.fn(),
  updatePanningConfig: vi.fn(),
  updateShortcuts: vi.fn(),
  setShortcutsPaused: vi.fn(),
}));

vi.mock("../services/settings-store", () => ({
  saveEngineSettings: vi.fn(),
  saveShortcuts: vi.fn(),
}));

vi.mock("../services/autostart", () => ({
  isAutostartEnabled: vi.fn(),
  setAutostartEnabled: vi.fn(),
}));

const DEFAULT_LIMITS: ZoomLimits = {
  minScale: 1.0,
  defaultScale: 2.0,
  maxScale: 5.0,
};

const DEFAULT_PANNING: PanningConfig = {
  mode: "center_follow",
  zoomStep: 0.25,
  zoomAlpha: 0.15,
  panAlpha: 0.25,
  edgeDeadzone: 0.15,
};

const DEFAULT_SHORTCUTS: ShortcutConfig = {
  toggle: "Ctrl+Alt+Z",
  zoomIn: "Ctrl+Alt+Up",
  zoomOut: "Ctrl+Alt+Down",
  reset: "Ctrl+Alt+0",
  pointerSizeUp: null,
  pointerSizeDown: null,
  pointerSizeReset: null,
};

function makeStatus(overrides: Partial<ZoomStatus> = {}): ZoomStatus {
  return {
    active: false,
    targetScale: 1.0,
    currentScale: 1.0,
    inputMapping: { state: "disabled", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
    config: DEFAULT_PANNING,
    limits: DEFAULT_LIMITS,
    ...overrides,
  };
}

function makeSnapshot(): EngineSettingsSnapshot {
  return {
    status: makeStatus(),
    shortcuts: DEFAULT_SHORTCUTS,
    constraints: {
      scaleEnvelope: { min: 1.0, max: 10.0, step: 0.1 },
      zoomStep: { min: 0.05, max: 2.0, step: 0.05 },
      alpha: { min: 0.01, max: 1.0, step: 0.01 },
      edgeDeadzone: { min: 0.05, max: 0.45, step: 0.01 },
      defaultLimits: DEFAULT_LIMITS,
      defaultPanning: DEFAULT_PANNING,
      defaultShortcuts: DEFAULT_SHORTCUTS,
    },
  };
}

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
  });
}

describe("useAppSettings", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
    vi.mocked(tauriCommands.getEngineSettings).mockResolvedValue(makeSnapshot());
    vi.mocked(autostartService.isAutostartEnabled).mockResolvedValue(true);
    vi.mocked(settingsStore.saveEngineSettings).mockResolvedValue(undefined);
    vi.mocked(settingsStore.saveShortcuts).mockResolvedValue(undefined);
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("hydrates settings, constraints, and autostart state on mount", async () => {
    const onStatusSynced = vi.fn();
    const { result } = renderHook(() => useAppSettings({ onStatusSynced }));
    await flushPromises();

    expect(result.current.isLoading).toBe(false);
    expect(result.current.limits).toEqual(DEFAULT_LIMITS);
    expect(result.current.panning).toEqual(DEFAULT_PANNING);
    expect(result.current.shortcuts).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.autostartEnabled).toBe(true);
    expect(onStatusSynced).toHaveBeenCalledWith(makeStatus());
  });

  it("throttles live previews without writing to disk and persists only on commit", async () => {
    const p1: PanningConfig = { ...DEFAULT_PANNING, zoomStep: 0.3 };
    const p2: PanningConfig = { ...DEFAULT_PANNING, zoomStep: 0.5 };
    const committed: PanningConfig = { ...DEFAULT_PANNING, zoomStep: 0.5 };

    vi.mocked(tauriCommands.updatePanningConfig)
      .mockResolvedValueOnce(makeStatus({ config: p1 }))
      .mockResolvedValueOnce(makeStatus({ config: p2 }))
      .mockResolvedValueOnce(makeStatus({ config: committed }));

    const { result } = renderHook(() => useAppSettings());
    await flushPromises();

    act(() => {
      result.current.previewPanningConfig(p1);
      result.current.previewPanningConfig(p2);
    });
    await flushPromises();

    expect(tauriCommands.updatePanningConfig).toHaveBeenCalledTimes(1);
    expect(tauriCommands.updatePanningConfig).toHaveBeenCalledWith(p1);
    expect(settingsStore.saveEngineSettings).not.toHaveBeenCalled();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60);
    });
    expect(tauriCommands.updatePanningConfig).toHaveBeenCalledTimes(2);
    expect(tauriCommands.updatePanningConfig).toHaveBeenLastCalledWith(p2);
    expect(settingsStore.saveEngineSettings).not.toHaveBeenCalled();

    await act(async () => {
      await result.current.commitPanningConfig(committed);
    });

    expect(settingsStore.saveEngineSettings).toHaveBeenCalledTimes(1);
    expect(settingsStore.saveEngineSettings).toHaveBeenCalledWith({
      limits: DEFAULT_LIMITS,
      panning: committed,
    });
  });

  it("reverts state and skips disk persistence if Rust rejects limits update", async () => {
    vi.mocked(tauriCommands.updateZoomLimits).mockRejectedValueOnce(
      new Error("Invalid bounds"),
    );

    const { result } = renderHook(() => useAppSettings());
    await flushPromises();

    await act(async () => {
      await result.current.commitZoomLimits({
        minScale: 4.0,
        defaultScale: 4.5,
        maxScale: 5.0,
      });
    });

    expect(result.current.limits).toEqual(DEFAULT_LIMITS);
    expect(result.current.error).toBe("Invalid bounds");
    expect(settingsStore.saveEngineSettings).not.toHaveBeenCalled();
  });

  it("persists shortcuts on Rust acceptance and skips disk write on rollback error", async () => {
    const nextShortcuts: ShortcutConfig = {
      ...DEFAULT_SHORTCUTS,
      toggle: "Ctrl+Shift+Z",
    };

    vi.mocked(tauriCommands.updateShortcuts)
      .mockResolvedValueOnce(nextShortcuts)
      .mockRejectedValueOnce("Hotkey already registered by another app");

    const { result } = renderHook(() => useAppSettings());
    await flushPromises();

    let ok = false;
    await act(async () => {
      ok = await result.current.updateShortcuts(nextShortcuts);
    });
    expect(ok).toBe(true);
    expect(result.current.shortcuts).toEqual(nextShortcuts);
    expect(settingsStore.saveShortcuts).toHaveBeenCalledWith(nextShortcuts);

    await act(async () => {
      ok = await result.current.updateShortcuts({
        ...nextShortcuts,
        zoomIn: "Ctrl+Shift+A",
      });
    });
    expect(ok).toBe(false);
    expect(result.current.shortcuts).toEqual(nextShortcuts);
    expect(result.current.shortcutError).toBe(
      "Hotkey already registered by another app",
    );
    expect(settingsStore.saveShortcuts).toHaveBeenCalledTimes(1);
  });

  it("reverts autostart state on plugin failure", async () => {
    vi.mocked(autostartService.setAutostartEnabled).mockRejectedValueOnce(
      "Registry access denied",
    );

    const { result } = renderHook(() => useAppSettings());
    await flushPromises();
    expect(result.current.autostartEnabled).toBe(true);

    await act(async () => {
      await result.current.toggleAutostart(false);
    });

    expect(result.current.autostartEnabled).toBe(true);
    expect(result.current.error).toBe("Registry access denied");
  });

  it("resets engine limits, panning, and shortcuts to defaults without altering autostart", async () => {
    const customLimits: ZoomLimits = {
      minScale: 1.5,
      defaultScale: 3.0,
      maxScale: 8.0,
    };
    const customPanning: PanningConfig = {
      ...DEFAULT_PANNING,
      mode: "edge_panning",
    };
    const customShortcuts: ShortcutConfig = {
      ...DEFAULT_SHORTCUTS,
      toggle: "Ctrl+Shift+Z",
    };

    vi.mocked(tauriCommands.getEngineSettings).mockResolvedValueOnce({
      ...makeSnapshot(),
      status: makeStatus({ limits: customLimits, config: customPanning }),
      shortcuts: customShortcuts,
    });
    vi.mocked(tauriCommands.updateZoomLimits).mockResolvedValueOnce(
      makeStatus({ config: customPanning }),
    );
    vi.mocked(tauriCommands.updatePanningConfig).mockResolvedValueOnce(
      makeStatus(),
    );
    vi.mocked(tauriCommands.updateShortcuts).mockResolvedValueOnce(
      DEFAULT_SHORTCUTS,
    );

    const { result } = renderHook(() => useAppSettings());
    await flushPromises();

    await act(async () => {
      await result.current.resetToDefaults();
    });

    expect(result.current.limits).toEqual(DEFAULT_LIMITS);
    expect(result.current.panning).toEqual(DEFAULT_PANNING);
    expect(result.current.shortcuts).toEqual(DEFAULT_SHORTCUTS);
    expect(result.current.autostartEnabled).toBe(true);
    expect(autostartService.setAutostartEnabled).not.toHaveBeenCalled();
    expect(settingsStore.saveEngineSettings).toHaveBeenCalledWith({
      limits: DEFAULT_LIMITS,
      panning: DEFAULT_PANNING,
    });
    expect(settingsStore.saveShortcuts).toHaveBeenCalledWith(DEFAULT_SHORTCUTS);
  });
});
