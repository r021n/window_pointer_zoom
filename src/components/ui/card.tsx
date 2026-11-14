import { forwardRef, type HTMLAttributes } from "react";
import { cn } from "../../lib/utils";

export const Card = forwardRef<HTMLDivElement, HTMLAttributes<HTMLDivElement>>(
  ({ className, ...props }, ref) => (
    <div
      ref={ref}
      className={cn(
        "border-t border-border text-card-foreground",
        className,
      )}
      {...props}
    />
  ),
);
Card.displayName = "Card";

export const CardHeader = forwardRef<
  HTMLDivElement,
  HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div
    ref={ref}
    className={cn("flex items-center justify-between gap-2 px-4 pt-4 pb-3", className)}
    {...props}
  />
));
CardHeader.displayName = "CardHeader";

export const CardTitle = forwardRef<
  HTMLHeadingElement,
  HTMLAttributes<HTMLHeadingElement>
>(({ className, ...props }, ref) => (
  <h2
    ref={ref}
    className={cn("text-sm font-semibold tracking-tight", className)}
    {...props}
  />
));
CardTitle.displayName = "CardTitle";

export const CardContent = forwardRef<
  HTMLDivElement,
  HTMLAttributes<HTMLDivElement>
>(({ className, ...props }, ref) => (
  <div ref={ref} className={cn("px-4 pb-4", className)} {...props} />
));
CardContent.displayName = "CardContent";
