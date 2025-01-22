import { RouterProvider, createHashRouter } from 'react-router-dom'
import { MainLayout } from './components/layout/MainLayout'
import { DetailsView } from './components/views/DetailsView'
import { CollectionsView } from './components/views/CollectionsView'

const router = createHashRouter([
  {
    path: '/',
    element: <MainLayout />,
    children: [
      {
        path: '/',
        element: <DetailsView />,
      },
      {
        path: '/collections',
        element: <CollectionsView />,
      },
    ],
  },
])

function App() {
  return <RouterProvider router={router} />
}

export default App
