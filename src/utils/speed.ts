import { clamp } from "../lib/utils";
import type { RangeBounds } from "../types/settings";

/**
 * Converts a lerp `alpha` factor (`[min, max]`) to an intuitive `0..100`
 * animation speed slider value using a logarithmic perceptual curve.
 */
export function alphaToSpeed(alpha: number, bounds: RangeBounds): number {
  const safeMin = Math.max(0.0001, bounds.min);
  const safeMax = Math.max(safeMin + 0.0001, bounds.max);
  const clamped = clamp(alpha, safeMin, safeMax);
  const ratio = Math.log(clamped / safeMin) / Math.log(safeMax / safeMin);
  return Math.round(clamp(ratio, 0, 1) * 100);
}

/**
 * Converts a `0..100` animation speed slider value back to a stepped lerp
 * `alpha` factor within `[bounds.min, bounds.max]`.
 */
export function speedToAlpha(speed: number, bounds: RangeBounds): number {
  const safeMin = Math.max(0.0001, bounds.min);
  const safeMax = Math.max(safeMin + 0.0001, bounds.max);
  const t = clamp(speed, 0, 100) / 100;
  const raw = safeMin * Math.pow(safeMax / safeMin, t);
  const step = bounds.step > 0 ? bounds.step : 0.01;
  const stepped = Math.round(raw / step) * step;
  return Number(clamp(stepped, bounds.min, bounds.max).toFixed(4));
}

/** Returns a concise human-readable label for a `0..100` speed value. */
export function describeSpeed(speed: number): string {
  if (speed <= 25) return "Cinematic";
  if (speed <= 65) return "Balanced";
  if (speed <= 90) return "Responsive";
  return "Instant";
}
