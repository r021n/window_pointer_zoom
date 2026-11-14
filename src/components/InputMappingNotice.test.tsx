import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { ZoomStatus } from "../types/settings";
import { InputMappingDiagnostics, InputMappingNotice } from "./InputMappingNotice";

const status: ZoomStatus = {
  active: true, currentScale: 2, targetScale: 2,
  limits: { minScale: 1, defaultScale: 2, maxScale: 5 },
  config: { mode: "center_follow", zoomStep: 0.25, zoomAlpha: 0.15, panAlpha: 0.25, edgeDeadzone: 0.15 },
  inputMapping: { state: "active", uiAccess: true, errorCode: null, uiAccessErrorCode: null },
};

afterEach(cleanup);

describe("InputMappingNotice", () => {
  it("explains denied input mapping and lets the user reset zoom", () => {
    const onReset = vi.fn();
    render(<InputMappingNotice status={{ ...status, inputMapping: { ...status.inputMapping, state: "denied", uiAccess: false } }} onReset={onReset} />);
    expect(screen.getByRole("alert").textContent).toContain("Clicks may miss their targets");
    fireEvent.click(screen.getByRole("button", { name: "Reset (1.00x)" }));
    expect(onReset).toHaveBeenCalledOnce();
  });

  it("reports mapping failures with formatted HRESULT codes even when UIAccess is granted", () => {
    render(<InputMappingNotice status={{ ...status, inputMapping: { ...status.inputMapping, state: "error", errorCode: -1 } }} onReset={vi.fn()} />);
    const alert = screen.getByRole("alert");
    expect(alert.textContent).toContain("could not update zoom or click mapping");
    expect(alert.textContent).toContain("Zoom/input error: 0xFFFFFFFF (-1)");
  });

  it("warns before zooming when the process lacks UIAccess", () => {
    render(<InputMappingNotice status={{ ...status, active: false, inputMapping: { ...status.inputMapping, state: "disabled", uiAccess: false } }} onReset={vi.fn()} />);
    expect(screen.getByRole("alert").textContent).toContain("Accurate clicking while zoomed is unavailable");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("does not warn when mapping is healthy", () => {
    render(<InputMappingNotice status={status} onReset={vi.fn()} />);
    expect(screen.queryByRole("alert")).toBeNull();
  });
});

describe("InputMappingDiagnostics", () => {
  it("renders unknown UIAccess and hides error lines when codes are null", () => {
    render(
      <InputMappingDiagnostics
        status={{
          ...status,
          inputMapping: { state: "pending", uiAccess: null, errorCode: null, uiAccessErrorCode: null },
        }}
      />,
    );
    expect(screen.getByText("Mouse input diagnostics")).toBeTruthy();
    expect(screen.getByText("Input mapping: pending")).toBeTruthy();
    expect(screen.getByText("UIAccess: unknown")).toBeTruthy();
    expect(screen.queryByText(/Zoom\/input error:/)).toBeNull();
    expect(screen.queryByText(/Access check error:/)).toBeNull();
  });

  it("formats non-null errorCode and uiAccessErrorCode as hex HRESULT and decimal", () => {
    render(
      <InputMappingDiagnostics
        status={{
          ...status,
          inputMapping: {
            state: "error",
            uiAccess: false,
            errorCode: -2147024891,
            uiAccessErrorCode: 5,
          },
        }}
      />,
    );
    expect(screen.getByText("Input mapping: error")).toBeTruthy();
    expect(screen.getByText("UIAccess: false")).toBeTruthy();
    expect(screen.getByText("Zoom/input error: 0x80070005 (-2147024891)")).toBeTruthy();
    expect(screen.getByText("Access check error: 0x00000005 (5)")).toBeTruthy();
  });
});
