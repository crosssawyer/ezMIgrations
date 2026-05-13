import * as React from "react";
import { cva } from "class-variance-authority";
import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center rounded-md border font-medium transition-colors",
  {
    variants: {
      variant: {
        default: "border-border bg-secondary text-foreground",
        outline: "border-border text-foreground",
        primary: "border-primary/30 bg-primary/10 text-primary",
        muted: "border-border bg-muted text-muted-foreground",
      },
      size: {
        default: "px-2 py-0.5 text-xs gap-1.5",
        xs: "h-4 px-1 py-0 text-[9px] gap-1",
        sm: "h-4 px-1.5 text-[10px] gap-1.5",
      },
    },
    defaultVariants: { variant: "default", size: "default" },
  }
);

function Badge({ className, variant, size, ...props }) {
  return <div className={cn(badgeVariants({ variant, size }), className)} {...props} />;
}

export { Badge, badgeVariants };
