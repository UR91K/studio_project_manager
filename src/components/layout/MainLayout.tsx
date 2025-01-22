import { useState, useEffect } from 'react'
import { Outlet, useNavigate, useLocation } from 'react-router-dom'
import { Sidebar } from './Sidebar'
import { SearchBar } from '@/components/layout/SearchBar'
import { StatusBar } from '@/components/layout/StatusBar'
import { ActivityBar } from '@/components/layout/ActivityBar'
import { ResizablePanel } from '@/components/layout/ResizablePanel'
import { TooltipProvider } from '@/components/ui/tooltip'

export function MainLayout() {
  const navigate = useNavigate()
  const location = useLocation()
  const [activeView, setActiveView] = useState('projects')
  const [isSidebarVisible, setSidebarVisible] = useState(true)

  // Update active view based on current route
  useEffect(() => {
    if (location.pathname === '/collections') {
      setActiveView('collections')
      setSidebarVisible(false)
    } else {
      setActiveView('projects')
      setSidebarVisible(true)
    }
  }, [location.pathname])

  return (
    <TooltipProvider>
      <div className="h-screen w-screen flex flex-col overflow-hidden bg-background">
        {/* Activity bar (always visible) */}
        <div className="flex-1 flex min-h-0">
          <ActivityBar 
            activeView={activeView}
            onItemClick={(view) => {
              if (view === 'collections') {
                navigate('/collections')
              } else {
                navigate('/')
              }
            }}
          />
          
          {/* View-specific content */}
          {activeView === 'projects' ? (
            // Projects View with search and folders
            <div className="flex-1 flex flex-col min-w-0">
              {/* Search bar - only in Projects view */}
              <SearchBar />
              
              <div className="flex-1 flex min-h-0">
                {/* Folders panel */}
                {isSidebarVisible && (
                  <ResizablePanel 
                    defaultWidth={300}
                    minWidth={200}
                    maxWidth={400}
                    className="border-r border-border/40"
                  >
                    <Sidebar />
                  </ResizablePanel>
                )}
                
                {/* Projects list/grid */}
                <main className="flex-1 overflow-auto min-w-0">
                  <Outlet />
                </main>
              </div>
            </div>
          ) : (
            // Collections View
            <div className="flex-1 min-w-0">
              <main className="h-full overflow-auto">
                <Outlet />
              </main>
            </div>
          )}
        </div>
        
        {/* Status bar (always visible) */}
        <StatusBar />
      </div>
    </TooltipProvider>
  )
} 