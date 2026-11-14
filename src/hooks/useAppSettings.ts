import { useCallback, useEffect, useRef, useState } from "react";
import { createThrottle, formatError } from "../lib/utils";
import {
  isAutostartEnabled,
  setAutostartEnabled,
} from "../services/autostart";
import { saveEngineSettings, saveShortcuts } from "../services/settings-store";
import {
  getEngineSettings,
  setShortcutsPaused as invokeSetShortcutsPaused,
  updatePanningConfig as invokeUpdatePanningConfig,
  updateShortcuts as invokeUpdateShortcuts,
  updateZoomLimits as invokeUpdateZoomLimits,
} from "../services/tauri-commands";
import type {
  ConfigConstraints,
  PanningConfig,
  ShortcutConfig,
  ZoomLimits,
  ZoomStatus,
} from "../types/settings";

const PREVIEW_THROTTLE_MS = 60;

export interface UseAppSettingsOptions {
  onStatusSynced?: (status: ZoomStatus) => void;
}

export interface AppSettingsController {
  limits: ZoomLimits | null;
  panning: PanningConfig | null;
  shortcuts: ShortcutConfig | null;
  constraints: ConfigConstraints | null;
  autostartEnabled: boolean;
  isLoading: boolean;
  isSaving: boolean;
  error: string | null;
  shortcutError: string | null;
  previewZoomLimits: (next: ZoomLimits) => void;
  commitZoomLimits: (next?: ZoomLimits) => Promise<void>;
  previewPanningConfig: (next: PanningConfig) => void;
  commitPanningConfig: (next?: PanningConfig) => Promise<void>;
  updateShortcuts: (next: ShortcutConfig) => Promise<boolean>;
  setShortcutsPaused: (paused: boolean) => Promise<void>;
  clearShortcutError: () => void;
  toggleAutostart: (enabled: boolean) => Promise<void>;
  resetToDefaults: () => Promise<void>;
  clearError: () => void;
}

/**
 * Hydrates persisted engine settings + constraints from Rust and OS autostart
 * state, providing throttled live preview and commit-only `settings.json` persistence.
 */
export function useAppSettings(
  options: UseAppSettingsOptions = {},
): AppSettingsController {
  const { onStatusSynced } = options;

  const [limits, setLimits] = useState<ZoomLimits | null>(null);
  const [panning, setPanning] = useState<PanningConfig | null>(null);
  const [shortcuts, setShortcuts] = useState<ShortcutConfig | null>(null);
  const [constraints, setConstraints] = useState<ConfigConstraints | null>(
    null,
  );
  const [autostartEnabled, setAutostartState] = useState(false);
  const [isLoading, setIsLoading] = useState(true);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [shortcutError, setShortcutError] = useState<string | null>(null);

  const mountedRef = useRef(true);
  const limitsSeqRef = useRef(0);
  const panningSeqRef = useRef(0);
  const acceptedLimitsRef = useRef<ZoomLimits | null>(null);
  const acceptedPanningRef = useRef<PanningConfig | null>(null);
  const latestDraftLimitsRef = useRef<ZoomLimits | null>(null);
  const latestDraftPanningRef = useRef<PanningConfig | null>(null);

  const onStatusSyncedRef = useRef(onStatusSynced);
  useEffect(() => {
    onStatusSyncedRef.current = onStatusSynced;
  }, [onStatusSynced]);

  const sendLimitsPreview = useCallback(async (next: ZoomLimits) => {
    const seq = ++limitsSeqRef.current;
    try {
      const status = await invokeUpdateZoomLimits(next);
      if (!mountedRef.current || seq !== limitsSeqRef.current) {
        return;
      }
      acceptedLimitsRef.current = status.limits;
      setLimits(status.limits);
      setError(null);
      onStatusSyncedRef.current?.(status);
    } catch (err) {
      if (!mountedRef.current || seq !== limitsSeqRef.current) {
        return;
      }
      if (acceptedLimitsRef.current) {
        setLimits(acceptedLimitsRef.current);
      }
      setError(formatError(err));
    }
  }, []);

  const sendPanningPreview = useCallback(async (next: PanningConfig) => {
    const seq = ++panningSeqRef.current;
    try {
      const status = await invokeUpdatePanningConfig(next);
      if (!mountedRef.current || seq !== panningSeqRef.current) {
        return;
      }
      acceptedPanningRef.current = status.config;
      setPanning(status.config);
      setError(null);
      onStatusSyncedRef.current?.(status);
    } catch (err) {
      if (!mountedRef.current || seq !== panningSeqRef.current) {
        return;
      }
      if (acceptedPanningRef.current) {
        setPanning(acceptedPanningRef.current);
      }
      setError(formatError(err));
    }
  }, []);

  const limitsThrottleRef = useRef<ReturnType<
    typeof createThrottle<[ZoomLimits]>
  > | null>(null);
  const panningThrottleRef = useRef<ReturnType<
    typeof createThrottle<[PanningConfig]>
  > | null>(null);

  useEffect(() => {
    mountedRef.current = true;
    limitsThrottleRef.current = createThrottle(
      (next: ZoomLimits) => void sendLimitsPreview(next),
      PREVIEW_THROTTLE_MS,
    );
    panningThrottleRef.current = createThrottle(
      (next: PanningConfig) => void sendPanningPreview(next),
      PREVIEW_THROTTLE_MS,
    );

    let cancelled = false;

    void Promise.allSettled([getEngineSettings(), isAutostartEnabled()]).then(
      ([snapshotResult, autostartResult]) => {
        if (cancelled || !mountedRef.current) {
          return;
        }

        if (snapshotResult.status === "fulfilled") {
          const snapshot = snapshotResult.value;
          acceptedLimitsRef.current = snapshot.status.limits;
          acceptedPanningRef.current = snapshot.status.config;
          latestDraftLimitsRef.current = snapshot.status.limits;
          latestDraftPanningRef.current = snapshot.status.config;

          setLimits(snapshot.status.limits);
          setPanning(snapshot.status.config);
          setShortcuts(snapshot.shortcuts);
          setConstraints(snapshot.constraints);
          onStatusSyncedRef.current?.(snapshot.status);
        } else {
          setError(formatError(snapshotResult.reason));
        }

        if (autostartResult.status === "fulfilled") {
          setAutostartState(autostartResult.value);
        }

        setIsLoading(false);
      },
    );

    return () => {
      cancelled = true;
      mountedRef.current = false;
      limitsThrottleRef.current?.cancel();
      panningThrottleRef.current?.cancel();
    };
  }, [sendLimitsPreview, sendPanningPreview]);

  const previewZoomLimits = useCallback(
    (next: ZoomLimits) => {
      latestDraftLimitsRef.current = next;
      setLimits(next);
      if (limitsThrottleRef.current) {
        limitsThrottleRef.current.call(next);
      } else {
        void sendLimitsPreview(next);
      }
    },
    [sendLimitsPreview],
  );

  const commitZoomLimits = useCallback(async (next?: ZoomLimits) => {
    const target = next ?? latestDraftLimitsRef.current ?? acceptedLimitsRef.current;
    if (!target) {
      return;
    }

    limitsThrottleRef.current?.cancel();
    latestDraftLimitsRef.current = target;
    setLimits(target);
    const seq = ++limitsSeqRef.current;
    setIsSaving(true);

    try {
      const status = await invokeUpdateZoomLimits(target);
      if (!mountedRef.current || seq !== limitsSeqRef.current) {
        return;
      }
      acceptedLimitsRef.current = status.limits;
      acceptedPanningRef.current = status.config;
      latestDraftLimitsRef.current = status.limits;
      setLimits(status.limits);
      setError(null);
      onStatusSyncedRef.current?.(status);

      await saveEngineSettings({
        limits: status.limits,
        panning: status.config,
      });
    } catch (err) {
      if (!mountedRef.current || seq !== limitsSeqRef.current) {
        return;
      }
      if (acceptedLimitsRef.current) {
        setLimits(acceptedLimitsRef.current);
        latestDraftLimitsRef.current = acceptedLimitsRef.current;
      }
      setError(formatError(err));
    } finally {
      if (mountedRef.current) {
        setIsSaving(false);
      }
    }
  }, []);

  const previewPanningConfig = useCallback(
    (next: PanningConfig) => {
      latestDraftPanningRef.current = next;
      setPanning(next);
      if (panningThrottleRef.current) {
        panningThrottleRef.current.call(next);
      } else {
        void sendPanningPreview(next);
      }
    },
    [sendPanningPreview],
  );

  const commitPanningConfig = useCallback(async (next?: PanningConfig) => {
    const target =
      next ?? latestDraftPanningRef.current ?? acceptedPanningRef.current;
    if (!target) {
      return;
    }

    panningThrottleRef.current?.cancel();
    latestDraftPanningRef.current = target;
    setPanning(target);
    const seq = ++panningSeqRef.current;
    setIsSaving(true);

    try {
      const status = await invokeUpdatePanningConfig(target);
      if (!mountedRef.current || seq !== panningSeqRef.current) {
        return;
      }
      acceptedLimitsRef.current = status.limits;
      acceptedPanningRef.current = status.config;
      latestDraftPanningRef.current = status.config;
      setPanning(status.config);
      setError(null);
      onStatusSyncedRef.current?.(status);

      await saveEngineSettings({
        limits: status.limits,
        panning: status.config,
      });
    } catch (err) {
      if (!mountedRef.current || seq !== panningSeqRef.current) {
        return;
      }
      if (acceptedPanningRef.current) {
        setPanning(acceptedPanningRef.current);
        latestDraftPanningRef.current = acceptedPanningRef.current;
      }
      setError(formatError(err));
    } finally {
      if (mountedRef.current) {
        setIsSaving(false);
      }
    }
  }, []);

  const updateShortcuts = useCallback(async (next: ShortcutConfig) => {
    setIsSaving(true);
    try {
      const accepted = await invokeUpdateShortcuts(next);
      if (!mountedRef.current) {
        return false;
      }
      setShortcuts(accepted);
      setShortcutError(null);
      await saveShortcuts(accepted);
      return true;
    } catch (err) {
      if (mountedRef.current) {
        setShortcutError(formatError(err, "Failed to update global shortcuts."));
      }
      return false;
    } finally {
      if (mountedRef.current) {
        setIsSaving(false);
      }
    }
  }, []);

  const setShortcutsPaused = useCallback(async (paused: boolean) => {
    try {
      await invokeSetShortcutsPaused(paused);
    } catch (err) {
      if (mountedRef.current) {
        setShortcutError(formatError(err));
      }
    }
  }, []);

  const clearShortcutError = useCallback(() => setShortcutError(null), []);

  const toggleAutostart = useCallback(
    async (enabled: boolean) => {
      const previous = autostartEnabled;
      setAutostartState(enabled);
      try {
        await setAutostartEnabled(enabled);
        if (mountedRef.current) {
          setError(null);
        }
      } catch (err) {
        if (mountedRef.current) {
          setAutostartState(previous);
          setError(formatError(err, "Failed to update launch-on-boot setting."));
        }
      }
    },
    [autostartEnabled],
  );

  const resetToDefaults = useCallback(async () => {
    if (!constraints) {
      return;
    }

    limitsThrottleRef.current?.cancel();
    panningThrottleRef.current?.cancel();
    const limitsSeq = ++limitsSeqRef.current;
    const panningSeq = ++panningSeqRef.current;
    setIsSaving(true);

    try {
      await invokeUpdateZoomLimits(constraints.defaultLimits);
      const latestStatus = await invokeUpdatePanningConfig(
        constraints.defaultPanning,
      );
      const acceptedShortcuts = await invokeUpdateShortcuts(
        constraints.defaultShortcuts,
      );

      if (
        !mountedRef.current ||
        limitsSeq !== limitsSeqRef.current ||
        panningSeq !== panningSeqRef.current
      ) {
        return;
      }

      acceptedLimitsRef.current = latestStatus.limits;
      acceptedPanningRef.current = latestStatus.config;
      latestDraftLimitsRef.current = latestStatus.limits;
      latestDraftPanningRef.current = latestStatus.config;

      setLimits(latestStatus.limits);
      setPanning(latestStatus.config);
      setShortcuts(acceptedShortcuts);
      setError(null);
      setShortcutError(null);
      onStatusSyncedRef.current?.(latestStatus);

      await saveEngineSettings({
        limits: latestStatus.limits,
        panning: latestStatus.config,
      });
      await saveShortcuts(acceptedShortcuts);
    } catch (err) {
      if (mountedRef.current) {
        setError(formatError(err, "Failed to reset settings to defaults."));
      }
    } finally {
      if (mountedRef.current) {
        setIsSaving(false);
      }
    }
  }, [constraints]);

  const clearError = useCallback(() => setError(null), []);

  return {
    limits,
    panning,
    shortcuts,
    constraints,
    autostartEnabled,
    isLoading,
    isSaving,
    error,
    shortcutError,
    previewZoomLimits,
    commitZoomLimits,
    previewPanningConfig,
    commitPanningConfig,
    updateShortcuts,
    setShortcutsPaused,
    clearShortcutError,
    toggleAutostart,
    resetToDefaults,
    clearError,
  };
}
