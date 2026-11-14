import type { PanningMode } from "../types/settings";

export function PanningPreview({ mode }: { mode: PanningMode }) {
  const edge = mode === "edge_panning";
  return (
    <svg viewBox="0 0 360 100" aria-hidden="true" className="h-28 w-full rounded-xl bg-muted/50 text-primary">
      <rect x="90" y="12" width="180" height="76" rx="6" fill="none" stroke="currentColor" opacity="0.35" />
      <rect x="110" y="26" width="140" height="48" rx="2" fill="none" stroke="currentColor" strokeDasharray={edge ? "4 4" : undefined} opacity="0.5" />
      <path d="M180 33v34M163 50h34" stroke="currentColor" opacity="0.25" />
      <path d="M0 0v18l5-5 7 0z" transform={`translate(${edge ? 243 : 180} 45)`} fill="currentColor" />
      {edge && <path d="M280 50h18m-5-5 5 5-5 5" fill="none" stroke="currentColor" />}
    </svg>
  );
}
