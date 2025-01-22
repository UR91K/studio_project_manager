import { useVirtualizer } from "@tanstack/react-virtual"
import {
  ColumnDef,
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  SortingState,
  useReactTable,
} from "@tanstack/react-table"
import { useRef, useState } from "react"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { ChevronDown, ChevronUp, Play, Plus, Pencil } from "lucide-react"
import { cn } from "@/lib/utils"
import { Input } from "@/components/ui/input"

// Update Project type to include filename and alias
type Project = {
  demo: boolean
  name: string
  filename: string
  alias?: string
  modified: string
  created: string
  lastScanned: string
  timeSignature: string
  keyScale: string
  duration: string
  abletonVersion: string
  plugins: string[]
  samples: string[]
}

// Define columns
const columns: ColumnDef<Project>[] = [
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
      const [alias, setAlias] = useState(row.original.alias)
      
      if (isEditing) {
        return (
          <div className="flex items-center gap-2">
            <Input
              autoFocus
              defaultValue={alias || ""}
              className="h-6 text-xs"
              onBlur={(e) => {
                setAlias(e.target.value)
                setIsEditing(false)
                // TODO: Save alias (name column) to database
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  setAlias(e.currentTarget.value)
                  setIsEditing(false)
                  // TODO: Save alias (name column) to database
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
              {alias || row.original.filename}
            </span>
            {alias && (
              <span className="truncate text-muted-foreground">
                {row.original.filename}
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
    accessorKey: "lastScanned",
    header: "Last Scanned",
    size: 150,
  },
  {
    accessorKey: "timeSignature",
    header: "Time",
    size: 80,
  },
  {
    accessorKey: "keyScale",
    header: "Key",
    size: 100,
  },
  {
    accessorKey: "duration",
    header: "Length",
    size: 80,
  },
  {
    accessorKey: "abletonVersion",
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
            <DropdownMenuItem key={index} className="text-xs">
              {plugin}
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
              {sample}
            </DropdownMenuItem>
          ))}
        </DropdownMenuContent>
      </DropdownMenu>
    ),
  },
]

// Sample data
const data: Project[] = [
  {
    demo: true,
    name: "Test Project 1",
    filename: "untitled_project_01.als",
    alias: "New House Track",
    modified: "2024-03-20",
    created: "2024-03-19",
    lastScanned: "2024-03-20",
    timeSignature: "4/4",
    keyScale: "C Major",
    duration: "3:45",
    abletonVersion: "11.3.13",
    plugins: ["Serum", "Vital", "FabFilter Pro-Q 3", "Valhalla Vintage Verb"],
    samples: ["kicks/kick_01.wav", "snares/snare_03.wav", "hats/hat_02.wav"],
  },
  {
    demo: false,
    name: "Test Project 2",
    filename: "house_beat_124bpm.als",
    modified: "2024-03-18",
    created: "2024-03-18",
    lastScanned: "2024-03-19",
    timeSignature: "3/4",
    keyScale: "G Minor",
    duration: "2:30",
    abletonVersion: "11.3.13",
    plugins: ["Massive", "Soundtoys Decapitator", "Omnisphere"],
    samples: ["loops/drum_loop_01.wav", "vocals/vocal_01.wav"],
  },
  {
    demo: false,
    name: "Test Project 3",
    filename: "ambient_sketch_03.als",
    alias: "Night Drive",
    modified: "2024-03-17",
    created: "2024-03-15",
    lastScanned: "2024-03-19",
    timeSignature: "4/4",
    keyScale: "F# Minor",
    duration: "5:20",
    abletonVersion: "11.3.13",
    plugins: ["Diva", "Valhalla Shimmer", "Pro-L 2"],
    samples: ["pads/ambient_01.wav", "textures/noise_02.wav"],
  },
]

export function DetailsView() {
  const [sorting, setSorting] = useState<SortingState>([])
  const tableContainerRef = useRef<HTMLDivElement>(null)

  const table = useReactTable({
    data,
    columns,
    columnResizeMode: "onChange",
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    state: {
      sorting,
    },
  })

  const { rows } = table.getRowModel()
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => 28, // Reduced row height
    overscan: 10,
  })

  return (
    <div className="h-full flex flex-col">
      <div 
        ref={tableContainerRef}
        className="flex-1 overflow-auto"
        style={{ 
          width: table.getTotalSize(),
        }}
      >
        <table className="w-full border-collapse">
          <thead className="sticky top-0 z-10">
            {table.getHeaderGroups().map(headerGroup => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map(header => (
                  <th
                    key={header.id}
                    style={{
                      width: header.getSize(),
                      position: "relative",
                    }}
                    className="border-y border-r border-border/40 bg-muted/50 p-2 text-left text-xs font-medium text-muted-foreground select-none first:border-l"
                  >
                    {flexRender(
                      header.column.columnDef.header,
                      header.getContext()
                    )}
                    <div
                      onMouseDown={header.getResizeHandler()}
                      onTouchStart={header.getResizeHandler()}
                      className={cn(
                        "absolute right-0 top-0 h-full w-1 cursor-col-resize hover:bg-accent",
                        header.column.getIsResizing() && "bg-accent"
                      )}
                    />
                  </th>
                ))}
              </tr>
            ))}
          </thead>
          <tbody>
            {rowVirtualizer.getVirtualItems().map(virtualRow => {
              const row = rows[virtualRow.index]
              return (
                <tr 
                  key={row.id}
                  className="hover:bg-muted/50"
                >
                  {row.getVisibleCells().map(cell => (
                    <td
                      key={cell.id}
                      style={{
                        width: cell.column.getSize(),
                      }}
                      className="border-b border-r border-border/40 p-2 text-xs first:border-l"
                    >
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext()
                      )}
                    </td>
                  ))}
                </tr>
              )
            })}
          </tbody>
        </table>
      </div>
    </div>
  )
} 