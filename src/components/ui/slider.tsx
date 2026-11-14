import { forwardRef, type ComponentPropsWithoutRef, type ElementRef } from "react";
import * as SliderPrimitive from "@radix-ui/react-slider";
import { cn } from "../../lib/utils";

export const Slider = forwardRef<
  ElementRef<typeof SliderPrimitive.Root>,
  ComponentPropsWithoutRef<typeof SliderPrimitive.Root>
>(
  (
    {
      className,
      disabled,
      "aria-label": ariaLabel,
      "aria-labelledby": ariaLabelledBy,
      ...props
    },
    ref,
  ) => (
    <SliderPrimitive.Root
      ref={ref}
      disabled={disabled}
      className={cn(
        "relative flex w-full touch-none select-none items-center py-3 data-[disabled]:opacity-45 data-[disabled]:cursor-not-allowed",
        className,
      )}
      {...props}
    >
      <SliderPrimitive.Track className="relative h-1.5 w-full grow overflow-hidden rounded-full bg-muted">
        <SliderPrimitive.Range className="absolute h-full bg-primary" />
      </SliderPrimitive.Track>
      <SliderPrimitive.Thumb
        aria-label={ariaLabel}
        aria-labelledby={ariaLabelledBy}
        aria-disabled={disabled || undefined}
        className="block h-5 w-5 rounded-full border-2 border-primary bg-card shadow-sm transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 focus-visible:ring-offset-background disabled:pointer-events-none cursor-grab active:cursor-grabbing"
      />
    </SliderPrimitive.Root>
  ),
);
Slider.displayName = SliderPrimitive.Root.displayName;
