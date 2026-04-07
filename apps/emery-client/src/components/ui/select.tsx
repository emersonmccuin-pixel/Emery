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
            "flex h-11 w-full rounded-none border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-2",
            "font-mono text-sm text-[var(--foreground)]",
            "shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)]",
            "transition-colors duration-150",
            "hover:border-[var(--input-hover-border)]",
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
