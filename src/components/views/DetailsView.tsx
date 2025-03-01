import { useEffect, useRef, useState } from 'react'
import { invoke } from '@tauri-apps/api/tauri'
import { useVirtualizer } from '@tanstack/react-virtual'
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  SortingState,
  useReactTable,
  ColumnDef,
} from '@tanstack/react-table'
import { cn } from '@/lib/utils'
import { columns } from './columns'
import { Project } from '@/types'

export function DetailsView() {
  const [projects, setProjects] = useState<Project[]>([])
  const [loading, setLoading] = useState(true)
  const [error, setError] = useState<string | null>(null)
  const [sorting, setSorting] = useState<SortingState>([])
  const [columnOrder, setColumnOrder] = useState<string[]>(
    columns.map(col => {
      const column = col as ColumnDef<Project>
      return String(column.id ?? '')
    })
  )
  const [columnSizing, setColumnSizing] = useState<Record<string, number>>({})
  const tableContainerRef = useRef<HTMLDivElement>(null)
  const [draggingColumnId, setDraggingColumnId] = useState<string | null>(null)
  const [dragOverColumnId, setDragOverColumnId] = useState<string | null>(null)
  const [resizingColumnId, setResizingColumnId] = useState<string | null>(null)
  const [initialMouseX, setInitialMouseX] = useState<number | null>(null)
  const [initialColumnWidth, setInitialColumnWidth] = useState<number | null>(null)

  // Fetch projects on mount
  useEffect(() => {
    async function fetchProjects() {
      try {
        setLoading(true)
        setError(null)
        const data = await invoke<Project[]>('list_projects')
        setProjects(data)
      } catch (err) {
        console.error('Failed to fetch projects:', err)
        setError('Failed to load projects')
      } finally {
        setLoading(false)
      }
    }

    fetchProjects()
  }, [])

  const table = useReactTable({
    data: projects,
    columns,
    state: {
      sorting,
      columnOrder,
      columnSizing,
    },
    columnResizeMode: "onChange",
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    onSortingChange: setSorting,
    onColumnOrderChange: setColumnOrder,
    onColumnSizingChange: setColumnSizing,
    enableColumnResizing: true,
    defaultColumn: {
      minSize: 40,
      maxSize: 1000,
      size: 150,
    },
  })

  const reorderColumn = (draggedColumnId: string, targetColumnId: string) => {
    const newColumnOrder = [...columnOrder]
    const currentPosition = newColumnOrder.indexOf(draggedColumnId)
    const newPosition = newColumnOrder.indexOf(targetColumnId)
    
    newColumnOrder.splice(currentPosition, 1)
    newColumnOrder.splice(newPosition, 0, draggedColumnId)
    
    setColumnOrder(newColumnOrder)
  }

  const handleDragStart = (e: React.DragEvent<HTMLDivElement>, columnId: string) => {
    e.dataTransfer.setData('text/plain', columnId)
    setDraggingColumnId(columnId)
  }

  const handleDragOver = (e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault()
  }

  const handleDrop = (e: React.DragEvent<HTMLDivElement>, targetColumnId: string) => {
    e.preventDefault()
    const draggedColumnId = e.dataTransfer.getData('text/plain')
    if (draggedColumnId && draggedColumnId !== targetColumnId) {
      reorderColumn(draggedColumnId, targetColumnId)
    }
    setDraggingColumnId(null)
  }

  const { rows } = table.getRowModel()
  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => 35,
    overscan: 5,
  })

  const virtualRows = rowVirtualizer.getVirtualItems()
  const totalSize = rowVirtualizer.getTotalSize()
  const paddingTop = virtualRows.length > 0 ? virtualRows[0].start : 0
  const paddingBottom = virtualRows.length > 0 ? totalSize - virtualRows[virtualRows.length - 1].end : 0

  if (loading) {
    return (
      <div className="h-full flex items-center justify-center">
        <span className="text-sm text-muted-foreground">Loading projects...</span>
      </div>
    )
  }

  if (error) {
    return (
      <div className="h-full flex items-center justify-center">
        <span className="text-sm text-destructive">{error}</span>
      </div>
    )
  }

  if (projects.length === 0) {
    return (
      <div className="h-full flex items-center justify-center">
        <span className="text-sm text-muted-foreground">No projects found</span>
      </div>
    )
  }

  return (
    <div className="h-full flex flex-col">
      <div 
        ref={tableContainerRef}
        className="flex-1 overflow-auto scrollbar-thin scrollbar-track-transparent scrollbar-thumb-border hover:scrollbar-thumb-border/80"
      >
        <table 
          className="border-collapse w-full relative"
          style={{ width: table.getTotalSize() }}
        >
          <thead className="sticky top-0 z-10">
            {table.getHeaderGroups().map(headerGroup => (
              <tr key={headerGroup.id}>
                {headerGroup.headers.map(header => {
                  const isDragging = draggingColumnId === header.id

                  return (
                    <th
                      key={header.id}
                      style={{
                        width: header.getSize(),
                        position: "relative",
                      }}
                      className={cn(
                        "border-y border-r border-border/40 bg-muted/50 p-0 text-left text-xs font-medium text-muted-foreground select-none first:border-l",
                        isDragging && "opacity-50"
                      )}
                    >
                      <div
                        draggable
                        onDragStart={(e) => handleDragStart(e, header.id)}
                        onDragOver={handleDragOver}
                        onDrop={(e) => handleDrop(e, header.id)}
                        className="flex h-full items-center gap-1 p-2 cursor-grab active:cursor-grabbing"
                      >
                        {flexRender(
                          header.column.columnDef.header,
                          header.getContext()
                        )}
                      </div>
                      {header.column.getCanResize() && (
                        <div
                          onMouseDown={header.getResizeHandler()}
                          onTouchStart={header.getResizeHandler()}
                          className={cn(
                            "absolute right-0 top-0 h-full w-1 cursor-col-resize hover:bg-accent",
                            header.column.getIsResizing() && "bg-accent w-[2px]"
                          )}
                        />
                      )}
                    </th>
                  )
                })}
              </tr>
            ))}
          </thead>
          <tbody>
            {paddingTop > 0 && (
              <tr>
                <td colSpan={table.getAllColumns().length} style={{ height: `${paddingTop}px` }} />
              </tr>
            )}
            {virtualRows.map(virtualRow => {
              const row = rows[virtualRow.index]
              return (
                <tr 
                  key={row.id}
                  className="hover:bg-muted/50"
                  data-index={virtualRow.index}
                >
                  {row.getVisibleCells().map(cell => (
                    <td
                      key={cell.id}
                      style={{
                        width: cell.column.getSize(),
                      }}
                      className="border-b border-r border-border/40 p-2 text-xs first:border-l whitespace-nowrap overflow-hidden text-ellipsis"
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
            {paddingBottom > 0 && (
              <tr>
                <td colSpan={table.getAllColumns().length} style={{ height: `${paddingBottom}px` }} />
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
} 