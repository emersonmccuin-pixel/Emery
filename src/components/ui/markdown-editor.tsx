import { useCallback, useEffect, useRef, useState } from 'react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { cn } from '@/lib/utils'

export interface MarkdownEditorProps {
  value: string
  onChange: (value: string) => void
  readonly?: boolean
  className?: string
}

export function MarkdownEditor({ value, onChange, readonly = false, className }: MarkdownEditorProps) {
  const [editing, setEditing] = useState(false)
  const [draft, setDraft] = useState(value)
  const textareaRef = useRef<HTMLTextAreaElement>(null)

  // Keep draft in sync if value changes externally while not editing
  useEffect(() => {
    if (!editing) {
      setDraft(value)
    }
  }, [value, editing])

  const enterEdit = useCallback(() => {
    if (readonly) return
    setDraft(value)
    setEditing(true)
  }, [readonly, value])

  const exitEdit = useCallback(
    (save: boolean) => {
      setEditing(false)
      if (save && draft !== value) {
        onChange(draft)
      } else {
        setDraft(value)
      }
    },
    [draft, value, onChange],
  )

  useEffect(() => {
    if (editing && textareaRef.current) {
      textareaRef.current.focus()
      const len = textareaRef.current.value.length
      textareaRef.current.setSelectionRange(len, len)
    }
  }, [editing])

  if (editing) {
    return (
      <textarea
        ref={textareaRef}
        className={cn(
          'w-full min-h-[360px] rounded border border-hud-cyan/30 bg-black/60 p-3 font-mono text-[11px] text-hud-cyan/90 outline-none focus:border-hud-cyan resize-y',
          className,
        )}
        value={draft}
        spellCheck={false}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={() => exitEdit(true)}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.preventDefault()
            exitEdit(false)
          } else if (e.key === 'Enter' && e.ctrlKey) {
            e.preventDefault()
            exitEdit(true)
          }
        }}
      />
    )
  }

  return (
    <div
      role={readonly ? undefined : 'button'}
      tabIndex={readonly ? undefined : 0}
      title={readonly ? undefined : 'Click to edit'}
      className={cn(
        'markdown-body min-h-[2rem] rounded',
        !readonly && 'cursor-text hover:bg-hud-cyan/5 transition-colors',
        className,
      )}
      onClick={enterEdit}
      onKeyDown={(e) => {
        if (!readonly && (e.key === 'Enter' || e.key === ' ')) {
          e.preventDefault()
          enterEdit()
        }
      }}
    >
      {value ? (
        <ReactMarkdown remarkPlugins={[remarkGfm]}>{value}</ReactMarkdown>
      ) : (
        <span className="text-muted-foreground text-[11px]">
          {readonly ? 'No content.' : 'Click to add content…'}
        </span>
      )}
    </div>
  )
}
