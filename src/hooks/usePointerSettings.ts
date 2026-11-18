import { useCallback, useEffect, useRef, useState } from "react";
import { formatError } from "../lib/utils";
import {
  applyDefaultPointerSize,
  getPointerSettings,
  resetPointerSize,
  subscribePointerStatus,
  updatePointerSettings,
} from "../services/tauri-commands";
import type { PointerSettings, PointerSettingsSnapshot } from "../types/settings";

export function usePointerSettings() {
  const [snapshot, setSnapshot] = useState<PointerSettingsSnapshot | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [isUpdating, setIsUpdating] = useState(false);
  const mounted = useRef(false);
  const updating = useRef(false);

  // A shortcut can publish a newer status while a command response is in flight.
  const acceptSnapshot = useCallback((next: PointerSettingsSnapshot) => {
    setSnapshot((current) =>
      current && current.revision > next.revision ? current : next,
    );
  }, []);

  useEffect(() => {
    mounted.current = true;
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    void subscribePointerStatus((next) => {
      if (!cancelled) {
        acceptSnapshot(next);
      }
    })
      .then((stopListening) => {
        if (cancelled) {
          stopListening();
        } else {
          unlisten = stopListening;
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(formatError(err, "Unable to listen for pointer shortcut updates."));
        }
      })
      .then(() => {
        if (!cancelled) {
          return getPointerSettings();
        }
      })
      .then((current) => {
        if (!cancelled && current) {
          acceptSnapshot(current);
        }
      })
      .catch((err) => {
        if (!cancelled) {
          setError(formatError(err, "Unable to load pointer settings."));
        }
      });

    return () => {
      cancelled = true;
      mounted.current = false;
      unlisten?.();
    };
  }, [acceptSnapshot]);

  const runAction = async (action: () => Promise<PointerSettingsSnapshot>) => {
    if (updating.current) {
      return;
    }

    updating.current = true;
    setIsUpdating(true);
    setError(null);

    try {
      const next = await action();
      if (mounted.current) {
        acceptSnapshot(next);
      }
    } catch (err) {
      if (mounted.current) {
        setError(formatError(err, "Unable to update pointer settings."));
      }
    } finally {
      updating.current = false;
      if (mounted.current) {
        setIsUpdating(false);
      }
    }
  };

  const updateSettings = (settings: PointerSettings) =>
    runAction(() => updatePointerSettings(settings));

  return {
    snapshot,
    error: error ?? snapshot?.error ?? null,
    isUpdating,
    updateSettings,
    applyDefaultSize: () => runAction(applyDefaultPointerSize),
    resetSize: () => runAction(resetPointerSize),
    resetPreferences: () =>
      runAction(async () => {
        const current = snapshot ?? await getPointerSettings();
        return updatePointerSettings({
          enabled: false,
          defaultSize: current.constraints.defaultSize,
          step: current.constraints.defaultStep,
          clickEffectEnabled: false,
        });
      }),
  };
}
