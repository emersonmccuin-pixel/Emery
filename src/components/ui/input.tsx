import * as React from 'react'
import { cn } from '@/lib/utils'

const Input = React.forwardRef<HTMLInputElement, React.ComponentProps<'input'>>(
  ({ className, type, ...props }, ref) => {
    return (
      <input
        type={type}
        className={cn(
          'flex h-7 w-full bg-transparent px-2 py-1 font-mono text-xs text-foreground',
          'border border-transparent rounded-sm',
          'placeholder:text-muted-foreground/50',
          'focus:outline-none focus:border-primary/40 focus:shadow-[0_0_6px_var(--glow-primary)]',
          'disabled:cursor-not-allowed disabled:opacity-50',
          'transition-all',
          className
        )}
        ref={ref}
        {...props}
      />
    )
  }
)
Input.displayName = 'Input'

export { Input }
