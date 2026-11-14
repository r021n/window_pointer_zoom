import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import * as autostartService from "./services/autostart";
import * as settingsStore from "./services/settings-store";
import type {
  EngineSettingsSnapshot,
  PanningConfig,
  ShortcutConfig,
  ZoomLimits,
  ZoomStatus,
} from "./types/settings";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

vi.mock("./services/settings-store", () => ({
  loadTheme: vi.fn(() => Promise.resolve("system")),
  saveTheme: vi.fn(() => Promise.resolve()),
  saveEngineSettings: vi.fn(() => Promise.resolve()),
  saveShortcuts: vi.fn(() => Promise.resolve()),
}));

vi.mock("./services/autostart", () => ({
  isAutostartEnabled: vi.fn(() => Promise.resolve(false)),
  setAutostartEnabled: vi.fn(() => Promise.resolve()),
}));

const mockedInvoke = vi.mocked(invoke);
const mockedListen = vi.mocked(listen);

const DEFAULT_CONFIG: PanningConfig = {
  mode: "center_follow",
  zoomStep: 0.25,
  zoomAlpha: 0.15,
  panAlpha: 0.25,
  edgeDeadzone: 0.15,
};

const DEFAULT_LIMITS: ZoomLimits = {
  minScale: 1.0,
  defaultScale: 2.0,
  maxScale: 5.0,
};

const DEFAULT_SHORTCUTS: ShortcutConfig = {
  toggle: "Ctrl+Alt+Z",
  zoomIn: "Ctrl+Alt+Up",
  zoomOut: "Ctrl+Alt+Down",
  reset: "Ctrl+Alt+0",
};

function makeStatus(overrides: Partial<ZoomStatus> = {}): ZoomStatus {
  return {
    active: false,
    targetScale: 1.0,
    currentScale: 1.0,
    inputMapping: { state: "disabled", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
    config: DEFAULT_CONFIG,
    limits: DEFAULT_LIMITS,
    ...overrides,
  };
}

function makeSnapshot(status = makeStatus()): EngineSettingsSnapshot {
  return {
    status,
    shortcuts: DEFAULT_SHORTCUTS,
    constraints: {
      scaleEnvelope: { min: 1.0, max: 10.0, step: 0.1 },
      zoomStep: { min: 0.05, max: 2.0, step: 0.05 },
      alpha: { min: 0.01, max: 1.0, step: 0.01 },
      edgeDeadzone: { min: 0.05, max: 0.45, step: 0.01 },
      defaultLimits: DEFAULT_LIMITS,
      defaultPanning: DEFAULT_CONFIG,
      defaultShortcuts: DEFAULT_SHORTCUTS,
    },
  };
}

function switchTab(name: string) {
  const tab = screen.getByRole("tab", { name });
  fireEvent.mouseDown(tab, { button: 0 });
  fireEvent.click(tab);
}

describe("App", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedListen.mockResolvedValue(vi.fn());
    vi.mocked(settingsStore.loadTheme).mockResolvedValue("system");
    vi.mocked(autostartService.isAutostartEnabled).mockResolvedValue(false);
  });

  afterEach(() => {
    cleanup();
  });

  it("renders controls tab, enforces 1.00x unmagnified floor when minScale > 1.0, and executes zoom commands", async () => {
    const elevatedLimits: ZoomLimits = {
      minScale: 1.5,
      defaultScale: 2.5,
      maxScale: 6.0,
    };
    const initial = makeStatus({ limits: elevatedLimits });
    const zoomed = makeStatus({
      active: true,
      currentScale: 1.5,
      targetScale: 1.5,
      limits: elevatedLimits,
    });
    const reset = makeStatus({ limits: elevatedLimits });

    mockedInvoke.mockImplementation(async (cmd) => {
      switch (cmd) {
        case "get_zoom_status":
          return initial;
        case "get_engine_settings":
          return makeSnapshot(initial);
        case "zoom_in":
          return zoomed;
        case "reset_zoom":
          return reset;
        default:
          return initial;
      }
    });

    render(<App />);

    expect(screen.getByText(/Connecting to fullscreen zoom engine/i)).toBeTruthy();

    await waitFor(() => {
      expect(screen.getByTestId("current-scale").textContent).toBe("1.00x");
    });

    const zoomOutBtn = screen.getByRole("button", {
      name: "Zoom Out",
    }) as HTMLButtonElement;
    const resetBtn = screen.getByRole("button", {
      name: "Reset (1.00x)",
    }) as HTMLButtonElement;
    expect(zoomOutBtn.disabled).toBe(true);
    expect(resetBtn.disabled).toBe(true);

    // Target slider keeps min=1.0 even when limits.minScale is 1.5
    const targetSlider = screen.getByRole("slider", {
      name: "Target Zoom Level",
    });
    expect(targetSlider.getAttribute("aria-valuemin")).toBe("1");
    expect(targetSlider.getAttribute("aria-valuemax")).toBe("6");

    fireEvent.click(screen.getByRole("button", { name: "Zoom In" }));
    await waitFor(() => {
      expect(screen.getByTestId("current-scale").textContent).toBe("1.50x");
      expect(screen.getByTestId("zoom-status-badge").textContent).toBe("Active");
    });
    expect(zoomOutBtn.disabled).toBe(false);
    expect(resetBtn.disabled).toBe(false);

    fireEvent.click(resetBtn);
    await waitFor(() => {
      expect(screen.getByTestId("current-scale").textContent).toBe("1.00x");
      expect(screen.getByTestId("zoom-status-badge").textContent).toBe("Inactive");
    });
  });

  it("switches to Motion tab and updates panning mode, deadzone state, and step presets", async () => {
    let current = makeStatus();

    mockedInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "get_engine_settings") {
        return makeSnapshot(current);
      }
      if (cmd === "update_panning_config") {
        const { config } = args as { config: PanningConfig };
        current = makeStatus({ config });
        return current;
      }
      return current;
    });

    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: "Motion" })).toBeTruthy();
    });

    switchTab("Motion");

    const deadzoneSlider = await screen.findByRole("slider", {
      name: "Edge Deadzone",
    });
    expect(deadzoneSlider.getAttribute("aria-disabled")).toBe("true");
    expect(deadzoneSlider.hasAttribute("data-disabled")).toBe(true);

    fireEvent.keyDown(screen.getByRole("radio", { name: "Center-locked" }), { key: "ArrowRight" });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
        config: { ...DEFAULT_CONFIG, mode: "edge_panning" },
      });
    });
    expect(deadzoneSlider.getAttribute("aria-disabled")).toBeNull();
    expect(deadzoneSlider.hasAttribute("data-disabled")).toBe(false);
    expect(settingsStore.saveEngineSettings).toHaveBeenCalled();

    switchTab("Zoom");
    fireEvent.click(screen.getByRole("button", { name: "0.5x" }));
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
        config: { ...DEFAULT_CONFIG, mode: "edge_panning", zoomStep: 0.5 },
      });
    });
  });

  it("manages shortcut recording pause lifecycle, duplicate validation, and system settings", async () => {
    const current = makeStatus();

    mockedInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "get_engine_settings") {
        return makeSnapshot(current);
      }
      if (cmd === "update_shortcuts") {
        return (args as { config: ShortcutConfig }).config;
      }
      if (cmd === "set_shortcuts_paused") {
        return undefined;
      }
      return current;
    });

    render(<App />);
    await waitFor(() => {
      expect(screen.getByRole("tab", { name: /Settings/i })).toBeTruthy();
    });

    switchTab("Settings");

    const toggleBtn = await screen.findByRole("button", {
      name: /Toggle Zoom shortcut/i,
    });
    fireEvent.click(toggleBtn);

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("set_shortcuts_paused", {
        paused: true,
      });
    });

    // Duplicate binding against Zoom In ("Ctrl+Alt+Up")
    fireEvent.keyDown(toggleBtn, {
      key: "ArrowUp",
      code: "ArrowUp",
      ctrlKey: true,
      altKey: true,
    });

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain("already assigned");
      expect(mockedInvoke).toHaveBeenCalledWith("set_shortcuts_paused", {
        paused: false,
      });
    });

    // Record valid unique shortcut
    fireEvent.click(toggleBtn);
    await waitFor(() => {
      expect(toggleBtn.textContent).toContain("Press keys");
    });

    fireEvent.keyDown(toggleBtn, {
      key: "m",
      code: "KeyM",
      ctrlKey: true,
      shiftKey: true,
    });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("update_shortcuts", {
        config: { ...DEFAULT_SHORTCUTS, toggle: "Ctrl+Shift+M" },
      });
    });
    expect(settingsStore.saveShortcuts).toHaveBeenCalledWith({
      ...DEFAULT_SHORTCUTS,
      toggle: "Ctrl+Shift+M",
    });

    // Start on Boot switch
    const autostartSwitch = screen.getByRole("switch", {
      name: "Start on Boot",
    });
    fireEvent.click(autostartSwitch);
    await waitFor(() => {
      expect(autostartService.setAutostartEnabled).toHaveBeenCalledWith(true);
    });

    // Reset to Defaults button
    fireEvent.click(screen.getByRole("button", { name: "Reset to Defaults" }));
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("update_zoom_limits", {
        limits: DEFAULT_LIMITS,
      });
      expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
        config: DEFAULT_CONFIG,
      });
      expect(mockedInvoke).toHaveBeenCalledWith("update_shortcuts", {
        config: DEFAULT_SHORTCUTS,
      });
    });
  });

  it("shows and dismisses error banner when an IPC call fails", async () => {
    mockedInvoke.mockRejectedValue(new Error("MagInitialize failed"));

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain(
        "MagInitialize failed",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("renders InputMappingNotice when denied, resets zoom from notice, and shows InputMappingDiagnostics in Settings", async () => {
    const deniedStatus = makeStatus({
      active: true,
      currentScale: 2.0,
      targetScale: 2.0,
      inputMapping: {
        state: "denied",
        uiAccess: false,
        errorCode: -2147024891,
        uiAccessErrorCode: null,
      },
    });
    const resetStatus = makeStatus({
      active: false,
      currentScale: 1.0,
      targetScale: 1.0,
      inputMapping: {
        state: "denied",
        uiAccess: false,
        errorCode: -2147024891,
        uiAccessErrorCode: null,
      },
    });

    mockedInvoke.mockImplementation(async (cmd) => {
      switch (cmd) {
        case "get_zoom_status":
          return deniedStatus;
        case "get_engine_settings":
          return makeSnapshot(deniedStatus);
        case "reset_zoom":
          return resetStatus;
        default:
          return deniedStatus;
      }
    });

    render(<App />);

    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("Clicks may miss their targets while zoomed.");
    expect(alert.textContent).toContain("0x80070005 (-2147024891)");

    fireEvent.click(within(alert).getByRole("button", { name: "Reset (1.00x)" }));
    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("reset_zoom");
      expect(screen.getByTestId("current-scale").textContent).toBe("1.00x");
    });

    switchTab("Settings");
    const diagnosticsSummary = await screen.findByText("Mouse input diagnostics");
    const diagnostics = diagnosticsSummary.closest("details");
    expect(diagnostics?.textContent).toContain("Input mapping: denied");
    expect(diagnostics?.textContent).toContain("UIAccess: false");
    expect(diagnostics?.textContent).toContain("Zoom/input error: 0x80070005 (-2147024891)");
  });
});
