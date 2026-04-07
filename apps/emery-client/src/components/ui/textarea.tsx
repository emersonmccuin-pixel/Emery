import * as React from "react";

import { cn } from "@/lib/utils";

export interface TextareaProps extends React.TextareaHTMLAttributes<HTMLTextAreaElement> {}

const Textarea = React.forwardRef<HTMLTextAreaElement, TextareaProps>(
  ({ className, ...props }, ref) => {
    return (
      <textarea
        ref={ref}
        className={cn(
          [
            "flex min-h-28 w-full rounded-none border border-[var(--input-border)] bg-[var(--input-bg)] px-3 py-3",
            "font-mono text-sm text-[var(--foreground)] placeholder:text-[var(--text-tertiary)]",
            "shadow-[inset_0_1px_2px_rgba(0,0,0,0.3)]",
            "transition-colors duration-150",
            "hover:border-[var(--input-hover-border)]",
            "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]",
            "focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--surface-base)]",
          ].join(" "),
          className,
        )}
        {...props}
      />
    );
  },
);
Textarea.displayName = "Textarea";

export { Textarea };
