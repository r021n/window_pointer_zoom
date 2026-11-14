import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as settingsStore from "../services/settings-store";
import { useTheme } from "./useTheme";

vi.mock("../services/settings-store", () => ({
  loadTheme: vi.fn(),
  saveTheme: vi.fn(),
}));

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("useTheme", () => {
  let changeListeners: Array<(event: MediaQueryListEvent) => void> = [];
  let currentMatches = false;

  beforeEach(() => {
    vi.resetAllMocks();
    changeListeners = [];
    currentMatches = false;
    document.documentElement.classList.remove("dark");

    vi.mocked(settingsStore.loadTheme).mockResolvedValue("system");
    vi.mocked(settingsStore.saveTheme).mockResolvedValue(undefined);

    vi.stubGlobal("matchMedia", (query: string) => ({
      matches: currentMatches,
      media: query,
      onchange: null,
      addEventListener: (
        _event: string,
        cb: (event: MediaQueryListEvent) => void,
      ) => {
        changeListeners.push(cb);
      },
      removeEventListener: (
        _event: string,
        cb: (event: MediaQueryListEvent) => void,
      ) => {
        changeListeners = changeListeners.filter((fn) => fn !== cb);
      },
      dispatchEvent: () => true,
    }));
  });

  afterEach(() => {
    cleanup();
    vi.unstubAllGlobals();
    document.documentElement.classList.remove("dark");
  });

  it("loads stored theme and applies .dark class on documentElement", async () => {
    vi.mocked(settingsStore.loadTheme).mockResolvedValueOnce("dark");

    const { result } = renderHook(() => useTheme());
    await flushPromises();

    expect(result.current.theme).toBe("dark");
    expect(result.current.resolvedTheme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("reacts to OS prefers-color-scheme changes when theme is system", async () => {
    currentMatches = false;
    const { result } = renderHook(() => useTheme());
    await flushPromises();

    expect(result.current.resolvedTheme).toBe("light");
    expect(document.documentElement.classList.contains("dark")).toBe(false);

    act(() => {
      for (const listener of changeListeners) {
        listener({ matches: true } as MediaQueryListEvent);
      }
    });

    expect(result.current.resolvedTheme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
  });

  it("selects and persists themes via settings-store", async () => {
    const { result } = renderHook(() => useTheme());
    await flushPromises();

    await act(async () => {
      await result.current.setTheme("light");
    });
    expect(result.current.theme).toBe("light");
    expect(settingsStore.saveTheme).toHaveBeenCalledWith("light");

    await act(async () => {
      await result.current.setTheme("dark");
    });
    expect(result.current.theme).toBe("dark");
    expect(document.documentElement.classList.contains("dark")).toBe(true);
    expect(settingsStore.saveTheme).toHaveBeenCalledWith("dark");
  });
});
