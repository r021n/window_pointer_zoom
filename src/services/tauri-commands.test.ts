import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import {
  getZoomStatus,
  resetZoom,
  setZoomLevel,
  updatePanningConfig,
  zoomIn,
  zoomOut,
  type PanningConfig,
  type ZoomStatus,
} from "./tauri-commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const SAMPLE_CONFIG: PanningConfig = {
  mode: "center_follow",
  zoomStep: 0.25,
  zoomAlpha: 0.15,
  panAlpha: 0.25,
  edgeDeadzone: 0.15,
};

const SAMPLE_STATUS: ZoomStatus = {
  active: false,
  targetScale: 1.0,
  currentScale: 1.0,
  config: SAMPLE_CONFIG,
  limits: { minScale: 1.0, maxScale: 5.0 },
};

describe("tauri-commands", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
    mockedInvoke.mockResolvedValue(SAMPLE_STATUS);
  });

  it("invokes get_zoom_status", async () => {
    const result = await getZoomStatus();
    expect(mockedInvoke).toHaveBeenCalledWith("get_zoom_status");
    expect(result).toEqual(SAMPLE_STATUS);
  });

  it("invokes zoom_in", async () => {
    await zoomIn();
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_in");
  });

  it("invokes zoom_out", async () => {
    await zoomOut();
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_out");
  });

  it("invokes reset_zoom", async () => {
    await resetZoom();
    expect(mockedInvoke).toHaveBeenCalledWith("reset_zoom");
  });

  it("invokes set_zoom_level with { scale }", async () => {
    await setZoomLevel(2.5);
    expect(mockedInvoke).toHaveBeenCalledWith("set_zoom_level", { scale: 2.5 });
  });

  it("invokes update_panning_config with { config }", async () => {
    const nextConfig: PanningConfig = {
      ...SAMPLE_CONFIG,
      mode: "edge_panning",
      edgeDeadzone: 0.2,
    };
    await updatePanningConfig(nextConfig);
    expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
      config: nextConfig,
    });
  });
});
