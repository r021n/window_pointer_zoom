import { act, cleanup, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import * as tauriCommands from "../services/tauri-commands";
import type { PanningConfig, ZoomStatus } from "../types/settings";
import { useZoomController } from "./useZoomController";

vi.mock("../services/tauri-commands", () => ({
  getZoomStatus: vi.fn(),
  zoomIn: vi.fn(),
  zoomOut: vi.fn(),
  resetZoom: vi.fn(),
  setZoomLevel: vi.fn(),
  subscribeZoomStatus: vi.fn(() => Promise.resolve(vi.fn())),
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
    inputMapping: { state: "disabled", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
    config: BASE_CONFIG,
    limits: { minScale: 1.0, defaultScale: 2.0, maxScale: 5.0 },
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
    vi.mocked(tauriCommands.subscribeZoomStatus).mockResolvedValue(vi.fn());
  });

  afterEach(() => {
    cleanup();
    vi.restoreAllMocks();
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

  it("polls every 100ms while zooming in and slows once settled", async () => {
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

  it("throttles rapid setZoomLevel calls and flushes the latest value", async () => {
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValueOnce(makeStatus());
    vi.mocked(tauriCommands.setZoomLevel)
      .mockResolvedValueOnce(makeStatus({ active: true, targetScale: 1.5, currentScale: 1.5 }))
      .mockResolvedValueOnce(makeStatus({ active: true, targetScale: 3.0, currentScale: 3.0 }));

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    act(() => {
      result.current.setZoomLevel(1.5);
      result.current.setZoomLevel(2.2);
      result.current.setZoomLevel(3.0);
    });
    await flushPromises();

    expect(tauriCommands.setZoomLevel).toHaveBeenCalledTimes(1);
    expect(tauriCommands.setZoomLevel).toHaveBeenLastCalledWith(1.5);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(60);
    });

    expect(tauriCommands.setZoomLevel).toHaveBeenCalledTimes(2);
    expect(tauriCommands.setZoomLevel).toHaveBeenLastCalledWith(3.0);
    expect(result.current.status?.targetScale).toBe(3.0);
  });

  it("detects input failures after the zoom scale has settled", async () => {
    const zoomed = makeStatus({ active: true, currentScale: 2, targetScale: 2,
      inputMapping: { state: "active", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
    });
    const failed = { ...zoomed, inputMapping: { ...zoomed.inputMapping, state: "error" as const, errorCode: -1 } };
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValueOnce(zoomed).mockResolvedValue(failed);
    const { result } = renderHook(() => useZoomController());
    await flushPromises();
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(result.current.status?.inputMapping.state).toBe("error");
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);
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

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);

    act(() => {
      result.current.setZoomLevel(2.0);
    });
    await flushPromises();
    expect(result.current.status).toEqual(nextTarget);

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

  it("updates settled state and triggers polling on external zoom-status-changed event", async () => {
    const initial = makeStatus();
    const externalToggle = makeStatus({
      active: true,
      currentScale: 1.0,
      targetScale: 2.0,
    });
    const animating = makeStatus({
      active: true,
      currentScale: 1.6,
      targetScale: 2.0,
    });
    const settled = makeStatus({
      active: true,
      currentScale: 2.0,
      targetScale: 2.0,
    });

    let emitStatus!: (status: ZoomStatus) => void;
    vi.mocked(tauriCommands.subscribeZoomStatus).mockImplementationOnce(
      async (onStatus) => {
        emitStatus = onStatus;
        return vi.fn<() => void>();
      },
    );
    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(animating)
      .mockResolvedValueOnce(settled);

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    act(() => {
      emitStatus(externalToggle);
    });
    expect(result.current.status).toEqual(externalToggle);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(animating);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(100);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(3);
    expect(result.current.status).toEqual(settled);

    await act(async () => {
      await vi.advanceTimersByTimeAsync(400);
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(3);
  });

  it("unlistens and removes visibilitychange listener on unmount", async () => {
    const unlistenStub = vi.fn();
    vi.mocked(tauriCommands.subscribeZoomStatus).mockResolvedValueOnce(
      unlistenStub,
    );
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValue(makeStatus());

    const { unmount } = renderHook(() => useZoomController());
    await flushPromises();

    unmount();
    expect(unlistenStub).toHaveBeenCalledTimes(1);

    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await flushPromises();

    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);
  });

  it("unlistens immediately if unmounted before async subscribeZoomStatus resolves", async () => {
    const unlistenStub = vi.fn();
    let resolveSubscribe!: (fn: () => void) => void;
    let emitStatus!: (status: ZoomStatus) => void;

    vi.mocked(tauriCommands.subscribeZoomStatus).mockImplementationOnce(
      (onStatus) => {
        emitStatus = onStatus;
        return new Promise((resolve) => {
          resolveSubscribe = resolve;
        });
      },
    );
    vi.mocked(tauriCommands.getZoomStatus).mockResolvedValueOnce(makeStatus());

    const { unmount } = renderHook(() => useZoomController());
    await flushPromises();

    unmount();
    expect(unlistenStub).not.toHaveBeenCalled();

    await act(async () => {
      resolveSubscribe(unlistenStub);
      await Promise.resolve();
    });
    expect(unlistenStub).toHaveBeenCalledTimes(1);

    act(() => {
      emitStatus(makeStatus({ active: true, targetScale: 2.0 }));
    });
  });

  it("refreshes status on visibilitychange only when visible and no mutations are pending", async () => {
    const initial = makeStatus();
    const refreshed = makeStatus({
      active: true,
      currentScale: 2.0,
      targetScale: 2.0,
    });

    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(initial)
      .mockResolvedValueOnce(refreshed);

    let resolveMutation!: (status: ZoomStatus) => void;
    vi.mocked(tauriCommands.zoomIn).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          resolveMutation = resolve;
        }),
    );

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    const visibilitySpy = vi
      .spyOn(document, "visibilityState", "get")
      .mockReturnValue("hidden");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await flushPromises();
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);

    let pendingAction!: Promise<void>;
    act(() => {
      pendingAction = result.current.zoomIn();
    });
    visibilitySpy.mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await flushPromises();
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(1);

    await act(async () => {
      resolveMutation(initial);
      await pendingAction;
    });

    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    await flushPromises();

    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);
    expect(result.current.status).toEqual(refreshed);
  });

  it("ignores stale getZoomStatus response superseded by zoom-status-changed", async () => {
    const initial = makeStatus();
    const eventStatus = makeStatus({
      active: true,
      currentScale: 2.0,
      targetScale: 2.0,
    });
    const staleRefresh = makeStatus({
      active: false,
      currentScale: 1.0,
      targetScale: 1.0,
    });

    let emitStatus!: (status: ZoomStatus) => void;
    vi.mocked(tauriCommands.subscribeZoomStatus).mockImplementationOnce(
      async (onStatus) => {
        emitStatus = onStatus;
        return vi.fn<() => void>();
      },
    );

    let resolveRefresh!: (status: ZoomStatus) => void;
    vi.mocked(tauriCommands.getZoomStatus)
      .mockResolvedValueOnce(initial)
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveRefresh = resolve;
          }),
      );

    const { result } = renderHook(() => useZoomController());
    await flushPromises();

    vi.spyOn(document, "visibilityState", "get").mockReturnValue("visible");
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"));
    });
    expect(tauriCommands.getZoomStatus).toHaveBeenCalledTimes(2);

    act(() => {
      emitStatus(eventStatus);
    });
    expect(result.current.status).toEqual(eventStatus);

    await act(async () => {
      resolveRefresh(staleRefresh);
      await Promise.resolve();
    });
    expect(result.current.status).toEqual(eventStatus);
  });
});
