import { useState } from 'react'
import { ColumnDef } from "@tanstack/react-table"
import { Project } from "@/types"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { ChevronDown, ChevronUp, Play, Plus, Pencil, Check, X } from "lucide-react"

export const columns: ColumnDef<Project>[] = [
  { 
    accessorKey: "demo",
    header: "",
    size: 40,
    enableSorting: false,
    cell: ({ row }) => (
      <Button 
        variant="ghost" 
        size="icon"
        className="h-6 w-6"
      >
        {row.original.demo ? 
          <Play className="h-3 w-3" /> : 
          <Plus className="h-3 w-3" />
        }
      </Button>
    ),
  },
  {
    accessorKey: "name",
    header: ({ column }) => (
      <div className="flex items-center gap-1">
        Name
        <Button
          variant="ghost"
          size="icon"
          className="h-4 w-4 p-0 opacity-50 hover:opacity-100"
          onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
        >
          {column.getIsSorted() === "asc" ? (
            <ChevronUp className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
        </Button>
      </div>
    ),
    size: 300,
    cell: ({ row }) => {
      const [isEditing, setIsEditing] = useState(false)
      const [name, setName] = useState(row.original.name)
      const baseFilename = row.original.filename.split(/[/\\]/).pop() || row.original.filename
      
      if (isEditing) {
        return (
          <div className="flex items-center gap-2">
            <Input
              autoFocus
              defaultValue={name}
              className="h-6 text-xs"
              onBlur={(e) => {
                setName(e.target.value)
                setIsEditing(false)
                // TODO: Save name to database
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  setName(e.currentTarget.value)
                  setIsEditing(false)
                  // TODO: Save name to database
                }
                if (e.key === "Escape") {
                  setIsEditing(false)
                }
              }}
            />
          </div>
        )
      }

      return (
        <div className="group flex items-center gap-2">
          <div className="flex items-center gap-2 min-w-0">
            <span className="truncate">
              {name || baseFilename}
            </span>
            {name && name !== baseFilename && (
              <span className="truncate text-muted-foreground">
                {baseFilename}
              </span>
            )}
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="h-5 w-5 opacity-0 group-hover:opacity-100 transition-opacity"
            onClick={() => setIsEditing(true)}
          >
            <Pencil className="h-3 w-3" />
          </Button>
        </div>
      )
    }
  },
  {
    accessorKey: "time_signature",
    header: "Time",
    size: 80,
  },
  {
    accessorKey: "key_scale",
    header: "Key",
    size: 100,
  },
  {
    accessorKey: "duration",
    header: "Length",
    size: 80,
  },
  {
    accessorKey: "ableton_version",
    header: "Version",
    size: 100,
  },
  {
    accessorKey: "plugins",
    header: "Plugins",
    size: 120,
    cell: ({ row }) => (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button 
            variant="ghost" 
            size="sm" 
            className="h-6 w-full justify-between font-normal text-xs"
          >
            {`${row.original.plugins.length} plugins`}
            <ChevronDown className="h-3 w-3 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-[200px]">
          {row.original.plugins.map((plugin, index) => (
            <DropdownMenuItem key={index} className="text-xs flex items-center justify-between gap-2">
              <span className="truncate">{plugin.name}</span>
              {plugin.installed ? (
                <Check className="h-3 w-3 text-green-500 flex-shrink-0" />
              ) : (
                <X className="h-3 w-3 text-destructive flex-shrink-0" />
              )}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    ),
  },
  {
    accessorKey: "samples",
    header: "Samples",
    size: 120,
    cell: ({ row }) => (
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button 
            variant="ghost" 
            size="sm" 
            className="h-6 w-full justify-between font-normal text-xs"
          >
            {`${row.original.samples.length} samples`}
            <ChevronDown className="h-3 w-3 opacity-50" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="start" className="w-[200px]">
          {row.original.samples.map((sample, index) => (
            <DropdownMenuItem key={index} className="text-xs">
              {sample.name}
              {!sample.is_present && (
                <span className="ml-1 text-destructive">(missing)</span>
              )}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    ),
  },
  {
    accessorKey: "modified",
    header: ({ column }) => (
      <div className="flex items-center gap-1">
        Modified
        <Button
          variant="ghost"
          size="icon"
          className="h-4 w-4 p-0 opacity-50 hover:opacity-100"
          onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
        >
          {column.getIsSorted() === "asc" ? (
            <ChevronUp className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
        </Button>
      </div>
    ),
    size: 150,
  },
  {
    accessorKey: "created",
    header: ({ column }) => (
      <div className="flex items-center gap-1">
        Created
        <Button
          variant="ghost"
          size="icon"
          className="h-4 w-4 p-0 opacity-50 hover:opacity-100"
          onClick={() => column.toggleSorting(column.getIsSorted() === "asc")}
        >
          {column.getIsSorted() === "asc" ? (
            <ChevronUp className="h-3 w-3" />
          ) : (
            <ChevronDown className="h-3 w-3" />
          )}
        </Button>
      </div>
    ),
    size: 150,
  },
  {
    accessorKey: "last_scanned",
    header: "Last Scanned",
    size: 150,
  },
] 