import { cn } from '@/lib/utils'
import { Button } from '@/components/ui/button'
import { FolderPlus, Folder, RefreshCw } from 'lucide-react'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { invoke } from '@tauri-apps/api/tauri'
import { useState } from 'react'
import { toast } from 'sonner'

export function Sidebar() {
  const [isScanning, setIsScanning] = useState(false)

  const handleRefresh = async () => {
    try {
      setIsScanning(true)
      await invoke('start_scan')
      toast.success('Started scanning projects')
    } catch (error) {
      toast.error(error as string)
    } finally {
      setIsScanning(false)
    }
  }

  return (
    <div className="h-full flex flex-col">
      {/* Header with actions */}
      <div className="p-2 flex items-center justify-between">
        <h2 className="text-sm font-medium">Project Folders</h2>
        <div className="flex gap-1">
          <Button 
            variant="ghost" 
            size="icon" 
            className="h-8 w-8"
            onClick={handleRefresh}
            disabled={isScanning}
          >
            <RefreshCw className={`h-4 w-4 ${isScanning ? 'animate-spin' : ''}`} />
          </Button>
          <Button variant="ghost" size="icon" className="h-8 w-8">
            <FolderPlus className="h-4 w-4" />
          </Button>
        </div>
      </div>
      
      <Separator className="opacity-40" />

      {/* Folders list */}
      <div className="flex-1 min-h-0">
        <ScrollArea className="h-full">
          <div className="p-2 space-y-1">
            {/* Example folders - replace with actual data */}
            {Array.from({ length: 2 }).map((_, i) => (
              <Button
                key={i}
                variant="ghost"
                size="sm"
                className="w-full justify-start font-normal"
              >
                <Folder className="w-4 h-4 mr-2 shrink-0" />
                <span className="truncate">Ableton Projects {i + 1}</span>
              </Button>
            ))}
          </div>
        </ScrollArea>
      </div>

      {/* Stats footer */}
      <div className="p-2 border-t border-border/40">
        <p className="text-xs text-muted-foreground">4 folders • 128 projects</p>
      </div>
    </div>
  )
} 