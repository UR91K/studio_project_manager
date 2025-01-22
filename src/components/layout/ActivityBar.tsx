import { cn } from "@/lib/utils"
import { Button } from "@/components/ui/button"
import { FolderOpen, Library, Settings } from "lucide-react"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"

interface ActivityBarProps {
  activeView?: string;
  onItemClick?: (item: string) => void;
}

export function ActivityBar({ activeView, onItemClick }: ActivityBarProps) {
  const items = [
    { icon: FolderOpen, label: "Projects", id: "projects" },
    { icon: Library, label: "Collections", id: "collections" },
  ]

  return (
    <div className="w-12 border-r border-border/40 bg-background flex flex-col items-center py-2">
      {items.map((item) => (
        <Tooltip key={item.id} delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className={cn(
                "h-12 w-12 relative text-muted-foreground hover:text-foreground",
                activeView === item.id && [
                  "bg-accent/50 text-foreground",
                  "after:absolute after:left-0 after:top-[25%] after:h-[50%] after:w-0.5 after:bg-foreground"
                ]
              )}
              onClick={() => onItemClick?.(item.id)}
            >
              <item.icon className="h-5 w-5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="right" className="text-xs">
            {item.label}
          </TooltipContent>
        </Tooltip>
      ))}
      <div className="flex-1" />
      <Tooltip delayDuration={0}>
        <TooltipTrigger asChild>
          <Button
            variant="ghost"
            size="icon"
            className="h-12 w-12 text-muted-foreground hover:text-foreground"
          >
            <Settings className="h-5 w-5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="right" className="text-xs">
          Settings
        </TooltipContent>
      </Tooltip>
    </div>
  )
} 