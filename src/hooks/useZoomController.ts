import { useCallback, useEffect, useRef, useState } from "react";
import { createThrottle, formatError } from "../lib/utils";
import {
  getZoomStatus,
  resetZoom as invokeResetZoom,
  setZoomLevel as invokeSetZoomLevel,
  subscribeZoomStatus,
  zoomIn as invokeZoomIn,
  zoomOut as invokeZoomOut,
} from "../services/tauri-commands";
import { SCALE_EPSILON, type ZoomStatus } from "../types/settings";

const POLL_INTERVAL_MS = 100;
const ZOOM_SLIDER_THROTTLE_MS = 60;

export interface ZoomController {
  status: ZoomStatus | null;
  error: string | null;
  isUpdating: boolean;
  zoomIn: () => Promise<void>;
  zoomOut: () => Promise<void>;
  resetZoom: () => Promise<void>;
  setZoomLevel: (scale: number) => void;
  syncStatus: (nextStatus: ZoomStatus) => void;
  clearError: () => void;
}

/**
 * Manages live zoom engine state, `zoom-status-changed` event subscriptions,
 * animation settling polls, and throttled `set_zoom_level` actions.
 */
export function useZoomController(): ZoomController {
  const [status, setStatus] = useState<ZoomStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);

  const mountedRef = useRef(true);
  // Monotonic sequence prevents stale responses (e.g. an older poll) from overwriting newer state.
  const statusSeqRef = useRef(0);
  const pendingMutationsRef = useRef(0);
  const pollInFlightRef = useRef(false);

  const applyResponse = useCallback((seq: number, nextStatus: ZoomStatus) => {
    if (!mountedRef.current || seq !== statusSeqRef.current) {
      return;
    }
    setStatus(nextStatus);
    setError(null);
  }, []);

  const applyError = useCallback((seq: number, err: unknown) => {
    if (!mountedRef.current || seq !== statusSeqRef.current) {
      return;
    }
    setError(formatError(err));
  }, []);

  const refreshStatus = useCallback(() => {
    const seq = ++statusSeqRef.current;

    return getZoomStatus()
      .then((nextStatus) => applyResponse(seq, nextStatus))
      .catch((err) => applyError(seq, err));
  }, [applyError, applyResponse]);

  const runAction = useCallback(
    async (action: () => Promise<ZoomStatus>) => {
      const seq = ++statusSeqRef.current;
      pendingMutationsRef.current += 1;
      setIsUpdating(true);

      try {
        const nextStatus = await action();
        applyResponse(seq, nextStatus);
      } catch (err) {
        applyError(seq, err);
      } finally {
        pendingMutationsRef.current -= 1;
        if (mountedRef.current && pendingMutationsRef.current === 0) {
          setIsUpdating(false);
        }
      }
    },
    [applyError, applyResponse],
  );

  const runActionRef = useRef(runAction);
  useEffect(() => {
    runActionRef.current = runAction;
  }, [runAction]);

  const throttledSetZoomRef = useRef<ReturnType<
    typeof createThrottle<[number]>
  > | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    throttledSetZoomRef.current = createThrottle((scale: number) => {
      void runActionRef.current(() => invokeSetZoomLevel(scale));
    }, ZOOM_SLIDER_THROTTLE_MS);

    void refreshStatus();

    void subscribeZoomStatus((nextStatus) => {
      if (cancelled) {
        return;
      }
      const seq = ++statusSeqRef.current;
      applyResponse(seq, nextStatus);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err) => {
        if (!cancelled) {
          applyError(statusSeqRef.current, err);
        }
      });

    const handleVisibilityChange = () => {
      if (
        document.visibilityState === "visible" &&
        pendingMutationsRef.current === 0
      ) {
        void refreshStatus();
      }
    };

    document.addEventListener("visibilitychange", handleVisibilityChange);

    return () => {
      cancelled = true;
      mountedRef.current = false;
      throttledSetZoomRef.current?.cancel();
      throttledSetZoomRef.current = null;
      unlisten?.();
      document.removeEventListener("visibilitychange", handleVisibilityChange);
    };
  }, [applyError, applyResponse, refreshStatus]);

  const shouldPoll = Boolean(
    status?.active &&
      Math.abs(status.currentScale - status.targetScale) > SCALE_EPSILON,
  );

  useEffect(() => {
    if (!shouldPoll) {
      return;
    }

    const intervalId = window.setInterval(() => {
      if (pollInFlightRef.current || pendingMutationsRef.current > 0) {
        return;
      }

      pollInFlightRef.current = true;

      void refreshStatus().finally(() => {
        pollInFlightRef.current = false;
      });
    }, POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [refreshStatus, shouldPoll]);

  const zoomIn = useCallback(() => {
    throttledSetZoomRef.current?.cancel();
    return runAction(invokeZoomIn);
  }, [runAction]);

  const zoomOut = useCallback(() => {
    throttledSetZoomRef.current?.cancel();
    return runAction(invokeZoomOut);
  }, [runAction]);

  const resetZoom = useCallback(() => {
    throttledSetZoomRef.current?.cancel();
    return runAction(invokeResetZoom);
  }, [runAction]);

  const setZoomLevel = useCallback((scale: number) => {
    if (throttledSetZoomRef.current) {
      throttledSetZoomRef.current.call(scale);
    } else {
      void runActionRef.current(() => invokeSetZoomLevel(scale));
    }
  }, []);

  const syncStatus = useCallback(
    (nextStatus: ZoomStatus) => {
      const seq = ++statusSeqRef.current;
      applyResponse(seq, nextStatus);
    },
    [applyResponse],
  );

  const clearError = useCallback(() => setError(null), []);

  return {
    status,
    error,
    isUpdating,
    zoomIn,
    zoomOut,
    resetZoom,
    setZoomLevel,
    syncStatus,
    clearError,
  };
}
