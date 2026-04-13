import { useRef, type Key, type ReactNode } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { ScrollArea } from '@/components/ui/scroll-area'

type VirtualListProps<T> = {
  className?: string
  emptyState?: ReactNode
  estimateSize: (item: T, index: number) => number
  getItemKey?: (item: T, index: number) => Key
  items: T[]
  overscan?: number
  renderItem: (item: T, index: number) => ReactNode
}

export function VirtualList<T>({
  className,
  emptyState,
  estimateSize,
  getItemKey,
  items,
  overscan = 8,
  renderItem,
}: VirtualListProps<T>) {
  const scrollRef = useRef<HTMLDivElement>(null)

  const virtualizer = useVirtualizer({
    count: items.length,
    estimateSize: (index) => estimateSize(items[index], index),
    getItemKey: (index) => getItemKey?.(items[index], index) ?? index,
    getScrollElement: () => scrollRef.current,
    overscan,
  })

  return (
    <ScrollArea ref={scrollRef} className={className}>
      {items.length === 0 ? (
        emptyState
      ) : (
        <div
          className="relative w-full"
          style={{
            height: `${virtualizer.getTotalSize()}px`,
          }}
        >
          {virtualizer.getVirtualItems().map((virtualItem) => (
            <div
              key={virtualItem.key}
              data-index={virtualItem.index}
              ref={virtualizer.measureElement}
              className="absolute left-0 top-0 w-full"
              style={{
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              {renderItem(items[virtualItem.index], virtualItem.index)}
            </div>
          ))}
        </div>
      )}
    </ScrollArea>
  )
}
