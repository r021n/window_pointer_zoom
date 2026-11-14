import { useCallback, useEffect, useRef, useState } from "react";
import {
  getZoomStatus,
  resetZoom as invokeResetZoom,
  setZoomLevel as invokeSetZoomLevel,
  updatePanningConfig as invokeUpdatePanningConfig,
  zoomIn as invokeZoomIn,
  zoomOut as invokeZoomOut,
  type PanningConfig,
  type ZoomStatus,
} from "../services/tauri-commands";

const POLL_INTERVAL_MS = 100;
const SCALE_SETTLE_EPSILON = 0.001;

function formatError(err: unknown): string {
  if (err instanceof Error && err.message) {
    return err.message;
  }
  if (typeof err === "string" && err.length > 0) {
    return err;
  }
  return "An unexpected error occurred while communicating with the zoom engine.";
}

export interface ZoomController {
  status: ZoomStatus | null;
  error: string | null;
  isUpdating: boolean;
  zoomIn: () => Promise<void>;
  zoomOut: () => Promise<void>;
  resetZoom: () => Promise<void>;
  setZoomLevel: (scale: number) => Promise<void>;
  updatePanningConfig: (config: PanningConfig) => Promise<void>;
  clearError: () => void;
}

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

  useEffect(() => {
    mountedRef.current = true;
    const seq = ++statusSeqRef.current;

    void getZoomStatus()
      .then((initialStatus) => applyResponse(seq, initialStatus))
      .catch((err) => applyError(seq, err));

    return () => {
      mountedRef.current = false;
    };
  }, [applyError, applyResponse]);

  const shouldPoll = Boolean(
    status?.active &&
      Math.abs(status.currentScale - status.targetScale) > SCALE_SETTLE_EPSILON,
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
      const seq = ++statusSeqRef.current;

      void getZoomStatus()
        .then((nextStatus) => applyResponse(seq, nextStatus))
        .catch((err) => applyError(seq, err))
        .finally(() => {
          pollInFlightRef.current = false;
        });
    }, POLL_INTERVAL_MS);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [applyError, applyResponse, shouldPoll]);

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

  const zoomIn = useCallback(() => runAction(invokeZoomIn), [runAction]);
  const zoomOut = useCallback(() => runAction(invokeZoomOut), [runAction]);
  const resetZoom = useCallback(() => runAction(invokeResetZoom), [runAction]);
  const setZoomLevel = useCallback(
    (scale: number) => runAction(() => invokeSetZoomLevel(scale)),
    [runAction],
  );
  const updatePanningConfig = useCallback(
    (config: PanningConfig) => runAction(() => invokeUpdatePanningConfig(config)),
    [runAction],
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
    updatePanningConfig,
    clearError,
  };
}
