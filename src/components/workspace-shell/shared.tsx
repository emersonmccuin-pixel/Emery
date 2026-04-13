import { PanelLoadingState } from '@/components/ui/panel-state'

export function shortCallSign(callSign: string): string {
  const match = callSign.match(/^(.+?)(-\d+)$/)
  if (!match) return callSign
  const [, namespace, number] = match
  return (namespace.length > 6 ? namespace.slice(0, 6) : namespace) + number
}

export function PanelFallback({ label }: { label: string }) {
  return (
    <div className="flex-1 min-h-0 p-6">
      <PanelLoadingState
        className="h-full min-h-[18rem]"
        detail="Preparing the selected operator surface."
        eyebrow="Workspace panel"
        title={label}
      />
    </div>
  )
}
