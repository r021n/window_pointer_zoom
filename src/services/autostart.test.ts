import { beforeEach, describe, expect, it, vi } from "vitest";
import { disable, enable, isEnabled } from "@tauri-apps/plugin-autostart";
import { isAutostartEnabled, setAutostartEnabled } from "./autostart";

vi.mock("@tauri-apps/plugin-autostart", () => ({
  isEnabled: vi.fn(),
  enable: vi.fn(),
  disable: vi.fn(),
}));

describe("autostart service", () => {
  beforeEach(() => {
    vi.resetAllMocks();
  });

  it("queries OS autostart state via plugin", async () => {
    vi.mocked(isEnabled).mockResolvedValueOnce(true);
    expect(await isAutostartEnabled()).toBe(true);
    expect(isEnabled).toHaveBeenCalledTimes(1);
  });

  it("calls enable when toggling on and disable when toggling off", async () => {
    vi.mocked(enable).mockResolvedValueOnce(undefined);
    vi.mocked(disable).mockResolvedValueOnce(undefined);

    await setAutostartEnabled(true);
    expect(enable).toHaveBeenCalledTimes(1);
    expect(disable).not.toHaveBeenCalled();

    await setAutostartEnabled(false);
    expect(disable).toHaveBeenCalledTimes(1);
  });
});
