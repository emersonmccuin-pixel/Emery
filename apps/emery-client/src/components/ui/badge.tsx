import * as React from "react";

import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const badgeVariants = cva(
  "inline-flex items-center border px-2 py-1 font-mono text-[10px] font-semibold uppercase tracking-[0.24em]",
  {
    variants: {
      variant: {
        default: "border-[var(--accent)]/40 bg-[var(--accent)]/10 text-[var(--accent)]",
        secondary: "border-[var(--accent-secondary)]/40 bg-[var(--accent-secondary)]/10 text-[var(--accent-secondary)]",
        outline: "border-[var(--border-default)] bg-transparent text-[var(--text-secondary)]",
      },
    },
    defaultVariants: {
      variant: "default",
    },
  },
);

function Badge({
  className,
  variant,
  ...props
}: React.HTMLAttributes<HTMLDivElement> & VariantProps<typeof badgeVariants>) {
  return <div className={cn(badgeVariants({ variant }), className)} {...props} />;
}

export { Badge, badgeVariants };
