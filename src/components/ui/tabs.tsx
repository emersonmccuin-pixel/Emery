import * as React from 'react'
import { cn } from '@/lib/utils'

type TabsContextValue = {
  value: string
  setValue: (value: string) => void
  idBase: string
}

const TabsContext = React.createContext<TabsContextValue | null>(null)

function useTabsContext(component: string): TabsContextValue {
  const ctx = React.useContext(TabsContext)
  if (!ctx) {
    throw new Error(`<${component}> must be rendered inside <Tabs>`)
  }
  return ctx
}

export interface TabsProps extends React.HTMLAttributes<HTMLDivElement> {
  value?: string
  defaultValue?: string
  onValueChange?: (value: string) => void
}

const Tabs = React.forwardRef<HTMLDivElement, TabsProps>(
  ({ value, defaultValue, onValueChange, className, children, ...props }, ref) => {
    const [internal, setInternal] = React.useState(defaultValue ?? '')
    const isControlled = value !== undefined
    const current = isControlled ? value : internal
    const idBase = React.useId()

    const setValue = React.useCallback(
      (next: string) => {
        if (!isControlled) setInternal(next)
        onValueChange?.(next)
      },
      [isControlled, onValueChange]
    )

    const ctx = React.useMemo<TabsContextValue>(
      () => ({ value: current, setValue, idBase }),
      [current, setValue, idBase]
    )

    return (
      <TabsContext.Provider value={ctx}>
        <div ref={ref} className={cn('flex flex-col min-h-0', className)} {...props}>
          {children}
        </div>
      </TabsContext.Provider>
    )
  }
)
Tabs.displayName = 'Tabs'

export interface TabsListProps extends React.HTMLAttributes<HTMLElement> {
  as?: 'nav' | 'div'
}

const TabsList = React.forwardRef<HTMLElement, TabsListProps>(
  ({ className, as = 'nav', children, ...props }, ref) => {
    const Comp = as as any
    return (
      <Comp
        ref={ref}
        role="tablist"
        className={cn('flex items-center h-full gap-6', className)}
        {...props}
      >
        {children}
      </Comp>
    )
  }
)
TabsList.displayName = 'TabsList'

export interface TabsTriggerProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  value: string
}

const TabsTrigger = React.forwardRef<HTMLButtonElement, TabsTriggerProps>(
  ({ value, className, children, onClick, ...props }, ref) => {
    const { value: current, setValue, idBase } = useTabsContext('TabsTrigger')
    const selected = current === value
    return (
      <button
        ref={ref}
        type="button"
        role="tab"
        id={`${idBase}-tab-${value}`}
        aria-selected={selected}
        aria-controls={`${idBase}-panel-${value}`}
        tabIndex={selected ? 0 : -1}
        data-state={selected ? 'active' : 'inactive'}
        className={cn('workspace-tab', selected && 'workspace-tab--active', className)}
        onClick={(event) => {
          onClick?.(event)
          if (!event.defaultPrevented) setValue(value)
        }}
        {...props}
      >
        {children}
      </button>
    )
  }
)
TabsTrigger.displayName = 'TabsTrigger'

export interface TabsContentProps extends React.HTMLAttributes<HTMLDivElement> {
  value: string
  forceMount?: boolean
}

const TabsContent = React.forwardRef<HTMLDivElement, TabsContentProps>(
  ({ value, forceMount, className, children, ...props }, ref) => {
    const { value: current, idBase } = useTabsContext('TabsContent')
    const selected = current === value
    if (!selected && !forceMount) return null
    return (
      <div
        ref={ref}
        role="tabpanel"
        id={`${idBase}-panel-${value}`}
        aria-labelledby={`${idBase}-tab-${value}`}
        hidden={!selected}
        data-state={selected ? 'active' : 'inactive'}
        className={cn('flex-1 min-h-0', className)}
        {...props}
      >
        {children}
      </div>
    )
  }
)
TabsContent.displayName = 'TabsContent'

export { Tabs, TabsList, TabsTrigger, TabsContent }
