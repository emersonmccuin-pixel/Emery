import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const buttonVariants = cva(
  'inline-flex items-center justify-center whitespace-nowrap font-mono text-xs uppercase tracking-wide transition-all disabled:pointer-events-none disabled:opacity-50 cursor-pointer',
  {
    variants: {
      variant: {
        default:
          'bg-primary/10 text-primary border border-primary/30 shadow-[inset_0_1px_0_rgba(255,255,255,0.06),0_1px_0_rgba(0,0,0,0.3)] hover:bg-primary/20 hover:shadow-[0_0_8px_var(--glow-primary)] active:shadow-[inset_0_2px_4px_rgba(0,0,0,0.4)] active:translate-y-px',
        destructive:
          'bg-destructive/10 text-destructive border border-destructive/30 hover:bg-destructive/20 hover:shadow-[0_0_8px_var(--glow-danger)]',
        outline:
          'border border-border text-muted-foreground hover:text-foreground hover:border-primary/40',
        ghost:
          'text-muted-foreground hover:text-foreground hover:bg-muted/20',
        link:
          'text-primary underline-offset-4 hover:underline',
      },
      size: {
        default: 'h-7 px-3 py-1',
        sm: 'h-6 px-2 py-0.5 text-[10px]',
        lg: 'h-8 px-4 py-1.5',
        icon: 'h-6 w-6 p-0',
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
)

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, ...props }, ref) => {
    return (
      <button
        className={cn(buttonVariants({ variant, size, className }))}
        ref={ref}
        {...props}
      />
    )
  }
)
Button.displayName = 'Button'

export { Button, buttonVariants }
