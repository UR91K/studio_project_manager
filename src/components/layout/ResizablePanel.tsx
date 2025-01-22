import { useRef, useState, useEffect } from 'react'
import { cn } from '@/lib/utils'

interface ResizablePanelProps {
  children: React.ReactNode
  defaultWidth?: number
  minWidth?: number
  maxWidth?: number
  className?: string
}

export function ResizablePanel({
  children,
  defaultWidth = 240,
  minWidth = 180,
  maxWidth = 480,
  className,
}: ResizablePanelProps) {
  const [width, setWidth] = useState(defaultWidth)
  const [isResizing, setIsResizing] = useState(false)
  const resizeRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const handleMouseMove = (e: MouseEvent) => {
      if (!isResizing) return
      
      const newWidth = e.clientX
      if (newWidth >= minWidth && newWidth <= maxWidth) {
        setWidth(newWidth)
      }
    }

    const handleMouseUp = () => {
      setIsResizing(false)
    }

    if (isResizing) {
      document.addEventListener('mousemove', handleMouseMove)
      document.addEventListener('mouseup', handleMouseUp)
    }

    return () => {
      document.removeEventListener('mousemove', handleMouseMove)
      document.removeEventListener('mouseup', handleMouseUp)
    }
  }, [isResizing, minWidth, maxWidth])

  return (
    <div
      className={cn("relative flex-shrink-0", className)}
      style={{ width: width }}
    >
      {children}
      <div
        ref={resizeRef}
        className="absolute right-0 top-0 bottom-0 w-1 cursor-col-resize hover:bg-border"
        onMouseDown={() => setIsResizing(true)}
      />
    </div>
  )
} 