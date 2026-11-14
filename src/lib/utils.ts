import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

/** Merges conditional class names and resolves conflicting Tailwind utilities. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs));
}

/** Formats unknown thrown values into a user-readable error string. */
export function formatError(
  err: unknown,
  fallback = "An unexpected error occurred while communicating with the zoom engine.",
): string {
  if (err instanceof Error && err.message) {
    return err.message;
  }
  if (typeof err === "string" && err.length > 0) {
    return err;
  }
  return fallback;
}

/** Formats a magnification scale factor consistently (e.g., `2.00x`). */
export function formatScale(scale: number): string {
  return `${scale.toFixed(2)}x`;
}

/** Clamps a numeric value to `[min, max]`. */
export function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export interface ThrottledCallback<Args extends unknown[]> {
  call: (...args: Args) => void;
  cancel: () => void;
}

/**
 * Creates a leading + trailing throttle so the first invocation runs
 * immediately and rapid subsequent calls coalesce into a trailing update.
 */
export function createThrottle<Args extends unknown[]>(
  fn: (...args: Args) => void,
  waitMs: number,
): ThrottledCallback<Args> {
  let lastRunAt: number | null = null;
  let timerId: number | undefined;
  let pendingArgs: Args | null = null;

  const cancel = () => {
    if (timerId !== undefined) {
      window.clearTimeout(timerId);
      timerId = undefined;
    }
    pendingArgs = null;
  };

  const call = (...args: Args) => {
    const now = Date.now();
    const elapsed = lastRunAt === null ? waitMs : now - lastRunAt;

    if (elapsed >= waitMs) {
      cancel();
      lastRunAt = now;
      fn(...args);
      return;
    }

    pendingArgs = args;
    if (timerId === undefined) {
      timerId = window.setTimeout(() => {
        timerId = undefined;
        const latest = pendingArgs;
        pendingArgs = null;
        if (latest) {
          lastRunAt = Date.now();
          fn(...latest);
        }
      }, waitMs - elapsed);
    }
  };

  return { call, cancel };
}
