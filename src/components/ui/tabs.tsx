import type { ReactNode } from 'react'

export type TabDefinition<T extends string = string> = {
  value: T
  label: string
  badge?: ReactNode
}

type TabsProps<T extends string> = {
  tabs: ReadonlyArray<TabDefinition<T>>
  value: T
  onChange: (value: T) => void
  className?: string
}

export function Tabs<T extends string>({ tabs, value, onChange, className }: TabsProps<T>) {
  return (
    <div
      role="tablist"
      className={`flex items-center gap-1 border-b border-hud-cyan/20 px-1 ${className ?? ''}`}
    >
      {tabs.map((tab) => {
        const active = tab.value === value
        return (
          <button
            key={tab.value}
            role="tab"
            aria-selected={active}
            type="button"
            onClick={() => onChange(tab.value)}
            className={`relative h-9 px-3 text-[10px] font-black uppercase tracking-widest transition-colors ${
              active
                ? 'text-hud-cyan'
                : 'text-hud-cyan/50 hover:text-hud-cyan/80'
            }`}
          >
            <span className="inline-flex items-center gap-2">
              {tab.label}
              {tab.badge}
            </span>
            {active ? (
              <span className="pointer-events-none absolute inset-x-2 bottom-0 h-0.5 bg-hud-cyan shadow-[0_0_8px_rgba(94,234,255,0.6)]" />
            ) : null}
          </button>
        )
      })}
    </div>
  )
}

type TabPanelProps = {
  when: boolean
  children: ReactNode
  className?: string
}

export function TabPanel({ when, children, className }: TabPanelProps) {
  if (!when) return null
  return <div className={className}>{children}</div>
}
