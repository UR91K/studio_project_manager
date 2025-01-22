import { Search, Hash, Code, SlidersHorizontal } from 'lucide-react'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { cn } from '@/lib/utils'
import { useState } from 'react'

export function SearchBar() {
  const [searchMode, setSearchMode] = useState<'text' | 'tags' | 'regex'>('text')

  return (
    <div className="h-12 border-b border-border/40 px-4 flex items-center justify-center bg-background">
      <div className="flex items-center gap-2 w-full max-w-2xl">
        {/* Search modes */}
        <div className="flex gap-1">
          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className={cn(
                  "h-8 w-8",
                  searchMode === 'text' && "bg-accent text-accent-foreground"
                )}
                onClick={() => setSearchMode('text')}
              >
                <Search className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs">
              Text Search
            </TooltipContent>
          </Tooltip>

          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className={cn(
                  "h-8 w-8",
                  searchMode === 'tags' && "bg-accent text-accent-foreground"
                )}
                onClick={() => setSearchMode('tags')}
              >
                <Hash className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs">
              Tag Search
            </TooltipContent>
          </Tooltip>

          <Tooltip delayDuration={0}>
            <TooltipTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                className={cn(
                  "h-8 w-8",
                  searchMode === 'regex' && "bg-accent text-accent-foreground"
                )}
                onClick={() => setSearchMode('regex')}
              >
                <Code className="h-4 w-4" />
              </Button>
            </TooltipTrigger>
            <TooltipContent side="bottom" className="text-xs">
              Regex Search
            </TooltipContent>
          </Tooltip>
        </div>

        {/* Search input */}
        <div className="flex-1 relative">
          <Input 
            placeholder={
              searchMode === 'text' ? "Search projects..." :
              searchMode === 'tags' ? "Search by tags..." :
              "Search with regex..."
            }
            className="pl-8 h-8"
          />
          <div className="absolute left-2 top-1/2 -translate-y-1/2 text-muted-foreground">
            {searchMode === 'text' && <Search className="h-4 w-4" />}
            {searchMode === 'tags' && <Hash className="h-4 w-4" />}
            {searchMode === 'regex' && <Code className="h-4 w-4" />}
          </div>
        </div>

        {/* Advanced filters button */}
        <Tooltip delayDuration={0}>
          <TooltipTrigger asChild>
            <Button
              variant="ghost"
              size="icon"
              className="h-8 w-8"
            >
              <SlidersHorizontal className="h-4 w-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" className="text-xs">
            Advanced Filters
          </TooltipContent>
        </Tooltip>
      </div>
    </div>
  )
} 