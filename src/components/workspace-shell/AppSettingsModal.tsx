import { Suspense, lazy } from 'react'
import { Settings, X } from 'lucide-react'
import { useShallow } from 'zustand/react/shallow'
import { useAppStore } from '../../store'
import { PanelFallback } from './shared'

const AppSettingsPanel = lazy(() => import('../AppSettingsPanel'))

function AppSettingsModal() {
  const { isAppSettingsOpen, appSettingsInitialTab } = useAppStore(
    useShallow((s) => ({
      isAppSettingsOpen: s.isAppSettingsOpen,
      appSettingsInitialTab: s.appSettingsInitialTab,
    })),
  )

  const { closeAppSettings } = useAppStore.getState()

  if (!isAppSettingsOpen) {
    return null
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/80 backdrop-blur-sm"
      role="dialog"
      aria-modal="true"
      onClick={() => closeAppSettings()}
    >
      <div
        className="relative flex flex-col w-[min(960px,92vw)] h-[min(720px,88vh)] bg-background border border-hud-cyan/40 rounded shadow-[0_0_40px_rgba(94,234,255,0.15)] overflow-hidden"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="flex items-center justify-between h-10 px-4 border-b border-hud-cyan/30 shrink-0">
          <div className="flex items-center gap-2">
            <Settings size={12} className="text-hud-cyan" />
            <span className="text-[10px] font-black uppercase tracking-[0.2em] text-hud-cyan">
              App Settings
            </span>
          </div>
          <button
            type="button"
            className="h-6 w-6 inline-flex items-center justify-center rounded text-hud-cyan/60 hover:text-hud-cyan hover:bg-hud-cyan/10"
            aria-label="Close App Settings"
            onClick={() => closeAppSettings()}
          >
            <X size={13} />
          </button>
        </div>
        <div className="flex-1 min-h-0">
          <Suspense fallback={<PanelFallback label="Loading settings..." />}>
            <AppSettingsPanel initialTab={appSettingsInitialTab} />
          </Suspense>
        </div>
      </div>
    </div>
  )
}

export default AppSettingsModal
