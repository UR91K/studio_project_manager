import { useEffect, useState } from 'react'
import { listen } from '@tauri-apps/api/event'

interface ScanProgress {
  status: string
  current: number
  total: number | null
  message: string
}

export function StatusBar() {
  const [progress, setProgress] = useState<ScanProgress>({
    status: 'ready',
    current: 0,
    total: null,
    message: 'Ready'
  })

  useEffect(() => {
    // Listen for scan progress events
    const unlisten = listen<ScanProgress>('scan:progress', (event) => {
      setProgress(event.payload)
    })

    return () => {
      unlisten.then(fn => fn()) // Cleanup listener
    }
  }, [])

  // Format progress message
  const progressMessage = progress.total 
    ? `${progress.message} (${progress.current}/${progress.total})`
    : progress.message

  return (
    <div className="h-6 border-t border-border/40 px-2 text-xs flex items-center bg-background/50 text-muted-foreground font-mono">
      <span>{progressMessage}</span>
    </div>
  )
}