import { formatScale } from "../lib/utils";
import { UNMAGNIFIED_SCALE, type InputMappingStatus, type ZoomStatus } from "../types/settings";
import { Button } from "./ui/button";

/** Formats a signed Win32/HRESULT code as `0xXXXXXXXX (decimal)`. */
function formatHresult(code: number): string {
  const hex = (code >>> 0).toString(16).toUpperCase().padStart(8, "0");
  return `0x${hex} (${code})`;
}

function DiagnosticDetails({
  mapping,
  className,
}: {
  mapping: InputMappingStatus;
  className?: string;
}) {
  return (
    <div className={className}>
      <p>Input mapping: {mapping.state}</p>
      <p>UIAccess: {mapping.uiAccess === null ? "unknown" : String(mapping.uiAccess)}</p>
      {mapping.errorCode !== null && (
        <p>Zoom/input error: {formatHresult(mapping.errorCode)}</p>
      )}
      {mapping.uiAccessErrorCode !== null && (
        <p>Access check error: {formatHresult(mapping.uiAccessErrorCode)}</p>
      )}
    </div>
  );
}

export function InputMappingDiagnostics({ status }: { status: ZoomStatus }) {
  return (
    <details className="mb-3 rounded-lg border px-3 py-2 text-xs text-muted-foreground">
      <summary className="cursor-pointer">Mouse input diagnostics</summary>
      <DiagnosticDetails mapping={status.inputMapping} className="mt-2" />
    </details>
  );
}

export function InputMappingNotice({ status, onReset }: {
  status: ZoomStatus;
  onReset: () => void;
}) {
  const mapping = status.inputMapping;
  const denied = mapping.state === "denied" || mapping.uiAccess === false;
  const failed = mapping.state === "error";
  if (!denied && !failed) return null;

  return (
    <div role="alert" className="mb-3 shrink-0 rounded-lg border border-destructive/40 bg-destructive/10 px-3 py-2 text-xs">
      <p className="font-semibold">{status.active ? "Clicks may miss their targets while zoomed." : "Accurate clicking while zoomed is unavailable."}</p>
      <p className="mt-1 text-muted-foreground">
        {denied
          ? "Use the signed installer for full mouse support. This copy can enlarge the screen, but Windows has not granted access to map clicks."
          : "Windows could not update zoom or click mapping. Reset zoom before selecting files, then restart the app."}
      </p>
      {status.active && (
        <Button size="sm" variant="outline" className="mt-2" onClick={onReset}>
          Reset ({formatScale(UNMAGNIFIED_SCALE)})
        </Button>
      )}
      <details className="mt-2 text-muted-foreground">
        <summary className="cursor-pointer">Diagnostic details</summary>
        <DiagnosticDetails mapping={mapping} className="mt-1" />
      </details>
    </div>
  );
}
