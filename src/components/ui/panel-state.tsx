import type { ReactNode } from 'react'
import { AlertTriangle, Inbox, LoaderCircle } from 'lucide-react'
import { cn } from '@/lib/utils'

type PanelTone = 'cyan' | 'green' | 'magenta' | 'amber' | 'destructive' | 'neutral'

const PANEL_TONES: Record<
  PanelTone,
  {
    banner: string
    eyebrow: string
    icon: string
    panel: string
  }
> = {
  cyan: {
    banner: 'border-hud-cyan/30 bg-hud-cyan/10 text-hud-cyan',
    eyebrow: 'text-hud-cyan/70',
    icon: 'border-hud-cyan/35 bg-hud-cyan/12 text-hud-cyan',
    panel:
      'border-hud-cyan/25 bg-[linear-gradient(135deg,rgba(58,240,224,0.14),rgba(58,240,224,0.04))] shadow-[inset_0_0_26px_rgba(58,240,224,0.08)]',
  },
  green: {
    banner: 'border-hud-green/30 bg-hud-green/10 text-hud-green',
    eyebrow: 'text-hud-green/70',
    icon: 'border-hud-green/35 bg-hud-green/12 text-hud-green',
    panel:
      'border-hud-green/25 bg-[linear-gradient(135deg,rgba(116,243,161,0.14),rgba(116,243,161,0.04))] shadow-[inset_0_0_26px_rgba(116,243,161,0.08)]',
  },
  magenta: {
    banner: 'border-hud-magenta/30 bg-hud-magenta/10 text-hud-magenta',
    eyebrow: 'text-hud-magenta/70',
    icon: 'border-hud-magenta/35 bg-hud-magenta/12 text-hud-magenta',
    panel:
      'border-hud-magenta/25 bg-[linear-gradient(135deg,rgba(255,51,153,0.14),rgba(255,51,153,0.04))] shadow-[inset_0_0_26px_rgba(255,51,153,0.08)]',
  },
  amber: {
    banner: 'border-hud-amber/30 bg-hud-amber/10 text-hud-amber',
    eyebrow: 'text-hud-amber/70',
    icon: 'border-hud-amber/35 bg-hud-amber/12 text-hud-amber',
    panel:
      'border-hud-amber/25 bg-[linear-gradient(135deg,rgba(240,192,64,0.14),rgba(240,192,64,0.04))] shadow-[inset_0_0_26px_rgba(240,192,64,0.08)]',
  },
  destructive: {
    banner: 'border-destructive/35 bg-destructive/12 text-destructive',
    eyebrow: 'text-destructive/75',
    icon: 'border-destructive/35 bg-destructive/12 text-destructive',
    panel:
      'border-destructive/25 bg-[linear-gradient(135deg,rgba(255,68,68,0.16),rgba(255,68,68,0.05))] shadow-[inset_0_0_26px_rgba(255,68,68,0.08)]',
  },
  neutral: {
    banner: 'border-white/15 bg-white/6 text-white/70',
    eyebrow: 'text-white/45',
    icon: 'border-white/15 bg-white/6 text-white/65',
    panel:
      'border-white/12 bg-[linear-gradient(135deg,rgba(255,255,255,0.07),rgba(255,255,255,0.02))] shadow-[inset_0_0_20px_rgba(255,255,255,0.03)]',
  },
}

type PanelStateProps = {
  action?: ReactNode
  align?: 'center' | 'start'
  className?: string
  compact?: boolean
  detail?: string
  eyebrow?: string
  icon?: ReactNode
  title: string
  tone?: PanelTone
}

export function PanelState({
  action,
  align = 'center',
  className,
  compact = false,
  detail,
  eyebrow,
  icon,
  title,
  tone = 'cyan',
}: PanelStateProps) {
  const palette = PANEL_TONES[tone]
  const centered = align === 'center'

  return (
    <div
      className={cn(
        'flex min-h-[14rem] flex-col justify-center gap-4 rounded-xl border text-white/85',
        centered ? 'items-center text-center' : 'items-start text-left',
        compact ? 'px-4 py-5' : 'px-6 py-8',
        palette.panel,
        className,
      )}
    >
      <div
        className={cn(
          'flex h-12 w-12 shrink-0 items-center justify-center rounded-full border',
          palette.icon,
        )}
      >
        {icon ?? <Inbox className="h-5 w-5" />}
      </div>

      <div className={cn('space-y-2', centered ? 'max-w-xl' : 'max-w-2xl')}>
        {eyebrow ? (
          <p className={cn('text-[10px] font-black uppercase tracking-[0.2em]', palette.eyebrow)}>
            {eyebrow}
          </p>
        ) : null}
        <h3 className="text-sm font-black tracking-[0.14em] text-white/92">{title}</h3>
        {detail ? <p className="text-xs leading-relaxed text-white/62">{detail}</p> : null}
      </div>

      {action ? (
        <div className={cn('pt-1', centered ? 'flex justify-center' : 'flex justify-start')}>
          {action}
        </div>
      ) : null}
    </div>
  )
}

type PanelLoadingStateProps = Omit<PanelStateProps, 'icon' | 'title'> & {
  title?: string
}

export function PanelLoadingState({
  detail = 'Fetching the latest workspace state.',
  title = 'Loading workspace state',
  tone = 'cyan',
  ...props
}: PanelLoadingStateProps) {
  return (
    <PanelState
      {...props}
      detail={detail}
      icon={<LoaderCircle className="h-5 w-5 animate-spin" />}
      title={title}
      tone={tone}
    />
  )
}

type PanelEmptyStateProps = PanelStateProps

export function PanelEmptyState(props: PanelEmptyStateProps) {
  return <PanelState {...props} icon={props.icon ?? <Inbox className="h-5 w-5" />} />
}

type PanelErrorStateProps = Omit<PanelStateProps, 'icon' | 'tone'> & {
  tone?: PanelTone
}

export function PanelErrorState({ tone = 'destructive', ...props }: PanelErrorStateProps) {
  return <PanelState {...props} icon={<AlertTriangle className="h-5 w-5" />} tone={tone} />
}

type PanelBannerProps = {
  className?: string
  message: string
  tone?: PanelTone
}

export function PanelBanner({
  className,
  message,
  tone = 'destructive',
}: PanelBannerProps) {
  const palette = PANEL_TONES[tone]

  return (
    <div
      className={cn(
        'flex items-start gap-2 border px-4 py-2.5 text-[10px] font-black uppercase tracking-[0.18em]',
        palette.banner,
        className,
      )}
    >
      <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0" />
      <span className="leading-relaxed">{message}</span>
    </div>
  )
}
