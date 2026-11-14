import { describe, expect, it } from "vitest";
import { alphaToSpeed, describeSpeed, speedToAlpha } from "./speed";

describe("speed utils", () => {
  const bounds = { min: 0.01, max: 1.0, step: 0.01 };

  it("maps boundary alpha values to 0 and 100", () => {
    expect(alphaToSpeed(0.01, bounds)).toBe(0);
    expect(alphaToSpeed(1.0, bounds)).toBe(100);
    expect(speedToAlpha(0, bounds)).toBe(0.01);
    expect(speedToAlpha(100, bounds)).toBe(1.0);
  });

  it("clamps out-of-range inputs cleanly", () => {
    expect(alphaToSpeed(-1, bounds)).toBe(0);
    expect(alphaToSpeed(5, bounds)).toBe(100);
    expect(speedToAlpha(-20, bounds)).toBe(0.01);
    expect(speedToAlpha(150, bounds)).toBe(1.0);
  });

  it("preserves monotonic ordering and round-trips defaults accurately", () => {
    const zoomSpeed = alphaToSpeed(0.15, bounds);
    const panSpeed = alphaToSpeed(0.25, bounds);
    expect(zoomSpeed).toBeGreaterThan(40);
    expect(panSpeed).toBeGreaterThan(zoomSpeed);
    expect(speedToAlpha(zoomSpeed, bounds)).toBeCloseTo(0.15, 1);
    expect(speedToAlpha(panSpeed, bounds)).toBeCloseTo(0.25, 1);
  });

  it("describes speed tiers clearly", () => {
    expect(describeSpeed(10)).toBe("Cinematic");
    expect(describeSpeed(55)).toBe("Balanced");
    expect(describeSpeed(80)).toBe("Responsive");
    expect(describeSpeed(100)).toBe("Instant");
  });
});
