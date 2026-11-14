import { beforeEach, describe, expect, it, vi } from "vitest";
import { load, type Store } from "@tauri-apps/plugin-store";
import {
  ENGINE_SETTINGS_KEY,
  SETTINGS_STORE_FILE,
  SHORTCUTS_KEY,
  THEME_KEY,
  loadTheme,
  saveEngineSettings,
  saveShortcuts,
  saveTheme,
} from "./settings-store";

vi.mock("@tauri-apps/plugin-store", () => ({
  load: vi.fn(),
}));

const mockedLoad = vi.mocked(load);

describe("settings-store", () => {
  const mockStore = {
    get: vi.fn(),
    set: vi.fn(),
    save: vi.fn(),
  };

  beforeEach(() => {
    vi.resetAllMocks();
    mockStore.set.mockResolvedValue(undefined);
    mockStore.save.mockResolvedValue(undefined);
    mockedLoad.mockResolvedValue(mockStore as unknown as Store);
  });

  it("saves engine settings to settings.json and flushes", async () => {
    const payload = {
      limits: { minScale: 1.0, defaultScale: 2.0, maxScale: 5.0 },
      panning: {
        mode: "center_follow" as const,
        zoomStep: 0.25,
        zoomAlpha: 0.15,
        panAlpha: 0.25,
        edgeDeadzone: 0.15,
      },
    };

    await saveEngineSettings(payload);

    expect(mockedLoad).toHaveBeenCalledWith(SETTINGS_STORE_FILE, {
      autoSave: false,
    });
    expect(mockStore.set).toHaveBeenCalledWith(ENGINE_SETTINGS_KEY, payload);
    expect(mockStore.save).toHaveBeenCalledTimes(1);
  });

  it("saves shortcuts to settings.json and flushes", async () => {
    const shortcuts = {
      toggle: "Ctrl+Alt+Z",
      zoomIn: "Ctrl+Alt+Up",
      zoomOut: "Ctrl+Alt+Down",
      reset: "Ctrl+Alt+0",
    };

    await saveShortcuts(shortcuts);

    expect(mockStore.set).toHaveBeenCalledWith(SHORTCUTS_KEY, shortcuts);
    expect(mockStore.save).toHaveBeenCalledTimes(1);
  });

  it("loads valid theme or falls back to system", async () => {
    mockStore.get.mockResolvedValueOnce("dark");
    expect(await loadTheme()).toBe("dark");
    expect(mockStore.get).toHaveBeenCalledWith(THEME_KEY);

    mockStore.get.mockResolvedValueOnce("invalid-value");
    expect(await loadTheme()).toBe("system");

    mockStore.get.mockResolvedValueOnce(undefined);
    expect(await loadTheme()).toBe("system");
  });

  it("saves theme preference and flushes", async () => {
    await saveTheme("light");
    expect(mockStore.set).toHaveBeenCalledWith(THEME_KEY, "light");
    expect(mockStore.save).toHaveBeenCalledTimes(1);
  });
});
