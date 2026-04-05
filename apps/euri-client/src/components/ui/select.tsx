import * as React from "react";

import { cn } from "@/lib/utils";

export interface SelectProps extends React.SelectHTMLAttributes<HTMLSelectElement> {}

const Select = React.forwardRef<HTMLSelectElement, SelectProps>(
  ({ className, children, ...props }, ref) => {
    return (
      <select
        ref={ref}
        className={cn(
          [
            "flex h-11 w-full rounded-none border border-[var(--border-default)] bg-[var(--surface-sunken)] px-3 py-2",
            "font-mono text-sm text-[var(--foreground)]",
            "shadow-[inset_0_0_0_1px_rgba(42,42,58,0.45),0_0_20px_rgba(0,212,255,0.05)]",
            "transition-colors duration-150",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]",
            "focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--surface-base)]",
          ].join(" "),
          className,
        )}
        {...props}
      >
        {children}
      </select>
    );
  },
);
Select.displayName = "Select";

export { Select };
