import * as React from "react";
import { cva, type VariantProps } from "class-variance-authority";

import { cn } from "@/lib/utils";

const buttonVariants = cva(
  [
    "inline-flex items-center justify-center gap-2 whitespace-nowrap",
    "font-mono text-xs font-semibold uppercase tracking-[0.24em]",
    "transition-all duration-150",
    "disabled:pointer-events-none disabled:opacity-50",
    "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[var(--accent)]",
    "focus-visible:ring-offset-2 focus-visible:ring-offset-[var(--surface-base)]",
    "min-h-11 px-4 py-2",
    "[clip-path:polygon(0_8px,8px_0,calc(100%-8px)_0,100%_8px,100%_calc(100%-8px),calc(100%-8px)_100%,8px_100%,0_calc(100%-8px))]",
  ].join(" "),
  {
    variants: {
      variant: {
        default: [
          "border border-[var(--accent)] bg-[var(--button-bg)] text-[var(--accent)]",
          "shadow-[0_0_0_1px_var(--accent-muted),0_0_16px_var(--accent-subtle)]",
          "hover:bg-[var(--accent)] hover:text-[var(--surface-base)] hover:shadow-[0_0_0_1px_var(--accent-muted),0_0_24px_var(--accent-muted)]",
        ].join(" "),
        secondary: [
          "border border-[var(--accent-secondary,var(--text-secondary))] bg-[var(--button-bg)] text-[var(--accent-secondary,var(--text-secondary))]",
          "shadow-[0_0_0_1px_var(--accent-subtle)]",
          "hover:bg-[var(--accent-secondary,var(--text-secondary))] hover:text-[var(--surface-base)] hover:shadow-[0_0_0_1px_var(--accent-muted)]",
        ].join(" "),
        ghost: [
          "border border-[var(--button-border)] bg-[var(--surface-raised)] text-[var(--text-secondary)]",
          "hover:border-[var(--accent-muted)] hover:bg-[var(--button-hover-bg)] hover:text-[var(--accent)]",
        ].join(" "),
        terminal: [
          "border border-[var(--accent)] bg-[var(--accent)] text-[var(--surface-base)]",
          "shadow-[0_0_0_1px_var(--accent-muted),0_0_20px_var(--accent-muted)]",
          "hover:brightness-110",
        ].join(" "),
      },
      size: {
        default: "min-h-11 px-4",
        sm: "min-h-9 px-3 text-[10px]",
        lg: "min-h-12 px-5 text-sm",
        icon: "size-11 px-0",
      },
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  },
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return <button className={cn(buttonVariants({ variant, size, className }))} ref={ref} {...props} />;
  },
);
Button.displayName = "Button";

export { Button, buttonVariants };
