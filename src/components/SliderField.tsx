import { useId, type ComponentProps } from "react";
import { Slider } from "./ui/slider";

type Props = Omit<ComponentProps<typeof Slider>, "aria-label" | "aria-labelledby"> & { label: string; displayValue: string };
export function SliderField({ label, displayValue, ...props }: Props) {
  const id = useId();
  return <div className="space-y-1">
    <div className="flex items-center justify-between gap-2 text-sm">
      <span id={id} className="font-medium">{label}</span>
      <span className="font-medium tabular-nums text-muted-foreground">{displayValue}</span>
    </div>
    <Slider aria-labelledby={id} {...props} />
  </div>;
}
