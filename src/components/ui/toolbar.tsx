import * as React from "react"
import * as ToolbarPrimitive from "@radix-ui/react-toolbar"
import { cn } from "@/lib/utils"

const Toolbar = React.forwardRef<
  React.ElementRef<typeof ToolbarPrimitive.Root>,
  React.ComponentPropsWithoutRef<typeof ToolbarPrimitive.Root>
>(({ className, ...props }, ref) => (
  <ToolbarPrimitive.Root
    ref={ref}
    className={cn(
      "flex h-10 items-center gap-1 rounded-md border bg-background p-1",
      className
    )}
    {...props}
  />
))
Toolbar.displayName = ToolbarPrimitive.Root.displayName

const ToolbarButton = React.forwardRef<
  React.ElementRef<typeof ToolbarPrimitive.Button>,
  React.ComponentPropsWithoutRef<typeof ToolbarPrimitive.Button>
>(({ className, ...props }, ref) => (
  <ToolbarPrimitive.Button
    ref={ref}
    className={cn(
      "inline-flex items-center justify-center rounded-sm px-2.5 py-1.5 text-sm font-medium ring-offset-background transition-colors hover:bg-muted hover:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
      className
    )}
    {...props}
  />
))
ToolbarButton.displayName = ToolbarPrimitive.Button.displayName

const ToolbarSeparator = React.forwardRef<
  React.ElementRef<typeof ToolbarPrimitive.Separator>,
  React.ComponentPropsWithoutRef<typeof ToolbarPrimitive.Separator>
>(({ className, ...props }, ref) => (
  <ToolbarPrimitive.Separator
    ref={ref}
    className={cn("mx-1 w-[1px] bg-border", className)}
    {...props}
  />
))
ToolbarSeparator.displayName = ToolbarPrimitive.Separator.displayName

const ToolbarLink = React.forwardRef<
  React.ElementRef<typeof ToolbarPrimitive.Link>,
  React.ComponentPropsWithoutRef<typeof ToolbarPrimitive.Link>
>(({ className, ...props }, ref) => (
  <ToolbarPrimitive.Link
    ref={ref}
    className={cn(
      "inline-flex items-center justify-center rounded-sm px-2.5 py-1.5 text-sm font-medium ring-offset-background transition-colors hover:bg-muted hover:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50",
      className
    )}
    {...props}
  />
))
ToolbarLink.displayName = ToolbarPrimitive.Link.displayName

export {
  Toolbar,
  ToolbarButton,
  ToolbarSeparator,
  ToolbarLink,
} 