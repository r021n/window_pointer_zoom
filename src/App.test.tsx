import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { invoke } from "@tauri-apps/api/core";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import App from "./App";
import type { PanningConfig, ZoomStatus } from "./services/tauri-commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

const DEFAULT_CONFIG: PanningConfig = {
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
    config: DEFAULT_CONFIG,
    limits: { minScale: 1.0, maxScale: 5.0 },
    ...overrides,
  };
}

describe("App", () => {
  beforeEach(() => {
    mockedInvoke.mockReset();
  });

  afterEach(() => {
    cleanup();
  });

  it("renders initial status, disables edgeDeadzone in center_follow mode, and handles zoom controls", async () => {
    const initial = makeStatus();
    const zoomed = makeStatus({
      active: true,
      currentScale: 1.25,
      targetScale: 1.25,
    });
    const sliderZoomed = makeStatus({
      active: true,
      currentScale: 2.5,
      targetScale: 2.5,
    });
    const reset = makeStatus({
      active: false,
      currentScale: 1.0,
      targetScale: 1.0,
    });

    mockedInvoke.mockImplementation(async (cmd) => {
      switch (cmd) {
        case "get_zoom_status":
          return initial;
        case "zoom_in":
          return zoomed;
        case "set_zoom_level":
          return sliderZoomed;
        case "zoom_out":
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

    expect(screen.getByTestId("zoom-status-badge").textContent).toBe("Inactive");

    const zoomOutBtn = screen.getByRole("button", {
      name: "Zoom Out",
    }) as HTMLButtonElement;
    const resetBtn = screen.getByRole("button", {
      name: "Reset (1.00x)",
    }) as HTMLButtonElement;
    expect(zoomOutBtn.disabled).toBe(true);
    expect(resetBtn.disabled).toBe(true);

    const deadzoneInput = screen.getByLabelText(
      "Edge Deadzone",
    ) as HTMLInputElement;
    expect(deadzoneInput.disabled).toBe(true);

    fireEvent.click(screen.getByRole("button", { name: "Zoom In" }));
    await waitFor(() => {
      expect(screen.getByTestId("target-scale").textContent).toBe("1.25x");
      expect(screen.getByTestId("zoom-status-badge").textContent).toBe("Active");
    });
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_in");

    const slider = screen.getByLabelText("Target Zoom Level") as HTMLInputElement;
    expect(slider.min).toBe("1");
    expect(slider.max).toBe("5");

    fireEvent.change(slider, { target: { value: "2.5" } });
    await waitFor(() => {
      expect(screen.getByTestId("target-scale").textContent).toBe("2.50x");
    });
    expect(mockedInvoke).toHaveBeenCalledWith("set_zoom_level", { scale: 2.5 });

    fireEvent.click(screen.getByRole("button", { name: "Zoom Out" }));
    await waitFor(() => {
      expect(screen.getByTestId("target-scale").textContent).toBe("1.25x");
    });
    expect(mockedInvoke).toHaveBeenCalledWith("zoom_out");

    fireEvent.click(screen.getByRole("button", { name: "Reset (1.00x)" }));
    await waitFor(() => {
      expect(screen.getByTestId("target-scale").textContent).toBe("1.00x");
      expect(screen.getByTestId("zoom-status-badge").textContent).toBe("Inactive");
    });
    expect(mockedInvoke).toHaveBeenCalledWith("reset_zoom");
  });

  it("updates panning mode and config parameters", async () => {
    let current = makeStatus();

    mockedInvoke.mockImplementation(async (cmd, args) => {
      if (cmd === "update_panning_config") {
        const { config } = args as { config: PanningConfig };
        current = makeStatus({ config });
        return current;
      }
      return current;
    });

    render(<App />);

    await waitFor(() => {
      expect(screen.getByLabelText("Panning Mode")).toBeTruthy();
    });

    const modeSelect = screen.getByLabelText("Panning Mode") as HTMLSelectElement;
    fireEvent.change(modeSelect, { target: { value: "edge_panning" } });

    await waitFor(() => {
      expect(
        (screen.getByLabelText("Edge Deadzone") as HTMLInputElement).disabled,
      ).toBe(false);
    });
    expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
      config: { ...DEFAULT_CONFIG, mode: "edge_panning" },
    });

    const zoomStepInput = screen.getByLabelText("Zoom Step") as HTMLInputElement;
    fireEvent.change(zoomStepInput, { target: { value: "0.5" } });

    await waitFor(() => {
      expect(mockedInvoke).toHaveBeenCalledWith("update_panning_config", {
        config: { ...DEFAULT_CONFIG, mode: "edge_panning", zoomStep: 0.5 },
      });
    });
  });

  it("shows and dismisses error banner when an IPC call fails", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("MagInitialize failed"));

    render(<App />);

    await waitFor(() => {
      expect(screen.getByRole("alert").textContent).toContain(
        "MagInitialize failed",
      );
    });

    fireEvent.click(screen.getByRole("button", { name: "Dismiss error" }));
    expect(screen.queryByRole("alert")).toBeNull();
  });
});
