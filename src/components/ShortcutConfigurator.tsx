import { ShortcutKeycaps } from "./ShortcutKeycaps";
import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type KeyboardEvent as ReactKeyboardEvent,
} from "react";
import { Keyboard } from "lucide-react";
import type { ShortcutConfig } from "../types/settings";
import {
  findDuplicateShortcutAction,
  formatKeyboardEventToAccelerator,
  type ShortcutActionKey,
} from "../utils/shortcut";
import { Button } from "./ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "./ui/card";

export interface ShortcutConfiguratorProps {
  shortcuts: ShortcutConfig;
  shortcutError: string | null;
  isSaving: boolean;
  onUpdateShortcuts: (next: ShortcutConfig) => Promise<boolean>;
  onSetShortcutsPaused: (paused: boolean) => Promise<void>;
  onClearShortcutError: () => void;
}

const SHORTCUT_ROWS: ReadonlyArray<{
  key: ShortcutActionKey;
  label: string;
}> = [
  { key: "toggle", label: "Toggle Zoom" },
  { key: "zoomIn", label: "Zoom In" },
  { key: "zoomOut", label: "Zoom Out" },
  { key: "reset", label: "Reset Zoom" },
];

export function ShortcutConfigurator({
  shortcuts,
  shortcutError,
  isSaving,
  onUpdateShortcuts,
  onSetShortcutsPaused,
  onClearShortcutError,
}: ShortcutConfiguratorProps) {
  const [recordingAction, setRecordingAction] =
    useState<ShortcutActionKey | null>(null);
  const [validationError, setValidationError] = useState<string | null>(null);

  const recordingRef = useRef<ShortcutActionKey | null>(null);
  const onSetShortcutsPausedRef = useRef(onSetShortcutsPaused);

  useEffect(() => {
    onSetShortcutsPausedRef.current = onSetShortcutsPaused;
  }, [onSetShortcutsPaused]);

  const stopRecording = useCallback(async () => {
    if (recordingRef.current === null) {
      return;
    }
    recordingRef.current = null;
    setRecordingAction(null);
    await onSetShortcutsPausedRef.current(false);
  }, []);

  useEffect(() => {
    return () => {
      if (recordingRef.current !== null) {
        recordingRef.current = null;
        void onSetShortcutsPausedRef.current(false);
      }
    };
  }, []);

  const handleStartRecording = async (action: ShortcutActionKey) => {
    if (recordingRef.current === action) {
      await stopRecording();
      return;
    }

    onClearShortcutError();
    setValidationError(null);

    if (recordingRef.current === null) {
      await onSetShortcutsPaused(true);
    }

    recordingRef.current = action;
    setRecordingAction(action);
  };

  const handleKeyDown = async (
    action: ShortcutActionKey,
    event: ReactKeyboardEvent<HTMLButtonElement>,
  ) => {
    if (recordingRef.current !== action) {
      return;
    }

    event.preventDefault();
    event.stopPropagation();

    if (
      event.key === "Escape" &&
      !event.ctrlKey &&
      !event.altKey &&
      !event.shiftKey &&
      !event.metaKey
    ) {
      setValidationError(null);
      await stopRecording();
      return;
    }

    const accelerator = formatKeyboardEventToAccelerator(event);
    if (!accelerator) {
      setValidationError(
        "Include a modifier key (Ctrl, Alt, Shift, or Win) or use a function key.",
      );
      return;
    }

    const duplicateAction = findDuplicateShortcutAction(
      shortcuts,
      action,
      accelerator,
    );
    if (duplicateAction) {
      setValidationError(
        `Shortcut '${accelerator}' is already assigned to ${SHORTCUT_ROWS.find((row) => row.key === duplicateAction)?.label}.`,
      );
      await stopRecording();
      return;
    }

    setValidationError(null);
    await stopRecording();
    await onUpdateShortcuts({ ...shortcuts, [action]: accelerator });
  };

  const handleBlur = (action: ShortcutActionKey) => {
    if (recordingRef.current === action) {
      setValidationError(null);
      void stopRecording();
    }
  };

  const displayedError = validationError ?? shortcutError;

  return (
    <Card aria-labelledby="shortcuts-heading">
      <CardHeader>
        <CardTitle id="shortcuts-heading" className="flex items-center gap-1.5">
          <Keyboard className="h-4 w-4 text-primary" />
          Global Shortcuts
        </CardTitle>
        <span className="text-xs text-muted-foreground">
          Click to record
        </span>
      </CardHeader>

      <CardContent className="space-y-2.5">
        {displayedError && (
          <div
            role="alert"
            className="rounded-lg border border-destructive/40 bg-destructive/10 px-2.5 py-2 text-xs text-destructive"
          >
            {displayedError}
          </div>
        )}

        <div className="space-y-2">
          {SHORTCUT_ROWS.map((row) => {
            const isRecording = recordingAction === row.key;
            return (
              <div
                key={row.key}
                className="flex items-center justify-between gap-3 border-b border-border py-3"
              >
                <span className="text-xs font-medium text-foreground">
                  {row.label}
                </span>
                <Button
                  size="sm"
                  variant={isRecording ? "default" : "outline"}
                  disabled={isSaving}
                  aria-label={`${row.label} shortcut: ${
                    isRecording ? "recording" : shortcuts[row.key]
                  }`}
                  onClick={() => void handleStartRecording(row.key)}
                  onKeyDown={(event) => void handleKeyDown(row.key, event)}
                  onBlur={() => handleBlur(row.key)}
                  className="min-w-[148px] font-mono text-xs"
                >
                  {isRecording
                    ? "Press keys… (Esc)"
                    : <ShortcutKeycaps shortcut={shortcuts[row.key]} />}
                </Button>
              </div>
            );
          })}
        </div>

        <p className="text-xs text-muted-foreground">
          Mouse wheel zoom remains fixed to{" "}
          <kbd className="rounded border border-border bg-muted px-1 py-0.5 font-mono text-[10px]">
            Ctrl + Alt + Scroll
          </kbd>
          .
        </p>
      </CardContent>
    </Card>
  );
}
