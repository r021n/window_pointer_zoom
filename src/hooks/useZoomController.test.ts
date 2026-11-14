import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as tauriCommands from "../services/tauri-commands";
import type { PanningConfig, ZoomStatus } from "../services/tauri-commands";
import { useZoomController } from "./useZoomController";

vi.mock("../services/tauri-commands", () => ({
  getZoomStatus: vi.fn(),
  zoomIn: vi.fn(),
  zoomOut: vi.fn(),
  resetZoom: vi.fn(),
  setZoomLevel: vi.fn(),
  updatePanningConfig: vi.fn(),
}));

const BASE_CONFIG: PanningConfig = {
  mode: "center_follow",
  zoomStep: 0.25,
  zoomAlpha: 0.15,
  panAlpha: 0.25,
  edgeDeadzone: 0.15,
};

function makeStatus(overrides: Partial<ZoomStatus> = {}): ZoomStatus {
  return {
    active: false,
    targetScale: 1.0,
    currentScale: 1.0,
    config: BASE_CONFIG,
    limits: { minScale: 1.0, maxScale: 5.0 },
    ...overrides,
  };
}

async function flushPromises(): Promise<void> {
  await act(async () => {
    await Promise.resolve();
  });
}

describe("useZoomController", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    vi.resetAllMocks();
  });

  afterEach(() => {
    cleanup();
    vi.useRealTimers();
  });

  it("loads initial zoom status on mount and does not poll when settled", async () => {
    const initial = makeStatus();
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValueOnce(initial);

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);
    expect(result.current.status).toEqual(initial);
    expect(result.current.error).toBeNull();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);
  });

  it("polls every 100ms while zooming in and stops once settled", async () => {
    const initial = makeStatus();
    const afterZoomIn = makeStatus({
      active: true,
      currentScale: 1.0,
      targetScale: 1.25,
    });
    const animating = makeStatus({
      active: true,
      currentScale: 1.15,
      targetScale: 1.25,
    });
    const settled = makeStatus({
      active: true,
      currentScale: 1.25,
      targetScale: 1.25,
    });

    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(animating)
      .mockResolvedValueOnce(settled);
    vi.mocked(tauriCommands.zoomIn).mockResolvedValueOnce(afterZoomIn);

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    await act(async () => {
      await result.current.zoomIn();
    });
    expect(result.current.status).toEqual(afterZoomIn);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status?.currentScale).toBe(1.15);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(3);
    expect(result.current.status?.currentScale).toBe(1.25);

    // Polling must stop after currentScale matches targetScale within 0.001
    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(3);
  });

  it("continues polling during resetZoom animation until settled at 1.0x", async () => {
    const zoomed = makeStatus({
      active: true,
      currentScale: 2.0,
      targetScale: 2.0,
    });
    const resetting = makeStatus({
      active: true,
      currentScale: 1.8,
      targetScale: 1.0,
    });
    const midReset = makeStatus({
      active: true,
      currentScale: 1.3,
      targetScale: 1.0,
    });
    const doneReset = makeStatus({
      active: false,
      currentScale: 1.0,
      targetScale: 1.0,
    });

    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(zoomed)
      .mockResolvedValueOnce(midReset)
      .mockResolvedValueOnce(doneReset);
    vi.mocked(tauriCommands.resetZoom).mockResolvedValueOnce(resetting);

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    await act(async () => {
      await result.current.resetZoom();
    });
    expect(result.current.status).toEqual(resetting);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(result.current.status).toEqual(midReset);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(result.current.status).toEqual(doneReset);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(3);
  });

  it("cleans up polling interval on unmount", async () => {
    const animating = makeStatus({
      active: true,
      currentScale: 1.1,
      targetScale: 2.0,
    });
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValue(animating);

    const { unmount } = renderHook(() => useZoomController());
    await flushPromises();

    unmount();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(500);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);
  });

  it("ignores stale poll response if a newer command resolves", async () => {
    const animating = makeStatus({
      active: true,
      currentScale: 1.1,
      targetScale: 1.5,
    });

    let resolvePoll!: (value: ZoomStatus) => void;
    const pendingPoll = new Promise<ZoomStatus>((resolve) => {
      resolvePoll = resolve;
    });

    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(animating)
      .mockReturnValueOnce(pendingPoll);

    const nextTarget = makeStatus({
      active: true,
      currentScale: 1.2,
      targetScale: 2.0,
    });
    vi.mocked(tauriCommands.setZoomLevel).mockResolvedValueOnce(nextTarget);

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    // Trigger one poll that remains in flight
    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);

    // Run a newer command while the poll is still pending
    await act(async () => {
      await result.current.setZoomLevel(2.0);
    });
    expect(result.current.status).toEqual(nextTarget);

    // Late poll resolves with older data and must NOT overwrite nextTarget
    await act(async () => {
      resolvePoll(
        makeStatus({ active: true, currentScale: 1.15, targetScale: 1.5 }),
      );
      await Promise.resolve();
    });
    expect(result.current.status).toEqual(nextTarget);
  });

  it("captures command errors and allows clearing them", async () => {
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValueOnce(makeStatus());
    vi.mocked(tauriCommands.zoomIn).mockRejectedValueOnce("Engine unavailable");

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    await act(async () => {
      await result.current.zoomIn();
    });
    expect(result.current.error).toBe("Engine unavailable");
    expect(result.current.isUpdating).toBe(false);

    act(() => {
      result.current.clearError();
    });
    expect(result.current.error).toBeNull();
  });
});
