export function ShortcutKeycaps({ shortcut }: { shortcut: string }) {
  return <span className="inline-flex flex-wrap gap-1">{shortcut.split("+").map((key, index) => <kbd key={`${key}-${index}`} className="rounded border border-border bg-background px-1.5 py-0.5 font-sans text-xs shadow-xs">{key}</kbd>)}</span>;
}
