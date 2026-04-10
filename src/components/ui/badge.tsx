import * as React from 'react'
import { cva, type VariantProps } from 'class-variance-authority'
import { cn } from '@/lib/utils'

const badgeVariants = cva(
  'inline-flex items-center font-mono text-[10px] uppercase tracking-widest leading-none',
  {
    variants: {
      variant: {
        default: 'text-muted-foreground',
        running:
          'text-primary bg-primary/10 border border-primary/30 px-1.5 py-0.5 rounded-full shadow-[0_0_6px_var(--glow-primary)]',
        offline:
          'text-muted-foreground bg-muted/10 border border-border px-1.5 py-0.5 rounded-full',
        destructive:
          'text-destructive bg-destructive/10 border border-destructive/30 px-1.5 py-0.5 rounded-full',
      },
    },
    defaultVariants: {
      variant: 'default',
    },
  }
)

export interface BadgeProps
  extends React.HTMLAttributes<HTMLSpanElement>,
    VariantProps<typeof badgeVariants> {}

function Badge({ className, variant, ...props }: BadgeProps) {
  return <span className={cn(badgeVariants({ variant }), className)} {...props} />
}

export { Badge, badgeVariants }
