import { Button } from "@/components/ui/button"
import { PlusCircle, MoreVertical } from "lucide-react"
import { ScrollArea } from "@/components/ui/scroll-area"

export function CollectionsView() {
  // Placeholder collections data
  const collections = [
    { id: 1, title: "New Album 2024", tracks: 8, lastModified: "2 days ago" },
    { id: 2, title: "EP Ideas", tracks: 4, lastModified: "1 week ago" },
    { id: 3, title: "Remixes", tracks: 6, lastModified: "2 weeks ago" },
    { id: 4, title: "Collaborations", tracks: 5, lastModified: "1 month ago" },
  ]

  return (
    <div className="h-full flex flex-col">
      {/* Header */}
      <div className="p-6 flex items-center justify-between border-b border-border/40">
        <div>
          <h1 className="text-2xl font-semibold mb-1">Collections</h1>
          <p className="text-sm text-muted-foreground">
            {collections.length} collections • {collections.reduce((acc, col) => acc + col.tracks, 0)} tracks
          </p>
        </div>
        <Button className="gap-2">
          <PlusCircle className="h-4 w-4" />
          New Collection
        </Button>
      </div>

      {/* Collections Grid */}
      <ScrollArea className="flex-1">
        <div className="p-6">
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 2xl:grid-cols-6 gap-4">
            {collections.map((collection) => (
              <div
                key={collection.id}
                className="group relative bg-card hover:bg-accent/50 rounded-md p-3 transition-colors"
              >
                {/* Collection Cover (placeholder) */}
                <div className="aspect-square mb-3 bg-accent/25 rounded-md flex items-center justify-center">
                  <span className="text-4xl font-bold text-accent-foreground/25">
                    {collection.title.charAt(0)}
                  </span>
                </div>

                {/* Collection Info */}
                <div className="space-y-1">
                  <div className="flex items-start justify-between">
                    <h3 className="font-medium truncate pr-2">{collection.title}</h3>
                    <Button
                      variant="ghost"
                      size="icon"
                      className="h-8 w-8 opacity-0 group-hover:opacity-100 transition-opacity"
                    >
                      <MoreVertical className="h-4 w-4" />
                    </Button>
                  </div>
                  <p className="text-xs text-muted-foreground">
                    {collection.tracks} tracks • {collection.lastModified}
                  </p>
                </div>
              </div>
            ))}
          </div>
        </div>
      </ScrollArea>
    </div>
  )
} 