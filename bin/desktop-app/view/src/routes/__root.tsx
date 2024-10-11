import { Toaster } from '@/components/ui/sonner'
import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/router-devtools'

export const Route = createRootRoute({
  component: () => (
    <div className="flex flex-col h-screen">
      <nav className="bg-accent text-white p-4">
        <div className="flex justify-between items-center">
          <div className="text-xl font-bold">Wavefy DLLM Demo</div>
          <div className="space-x-4">
            <Link
              to="/"
              className="hover:text-gray-300 transition-colors duration-200 [&.active]:text-blue-400 [&.active]:font-semibold"
            >
              Chat
            </Link>
            <Link
              to="/dashboard"
              className="hover:text-gray-300 transition-colors duration-200 [&.active]:text-blue-400 [&.active]:font-semibold"
            >
              Dashboard
            </Link>
          </div>
        </div>
      </nav>
      <div className="flex-1">
        <Outlet />
      </div>
      <Toaster />
      <TanStackRouterDevtools />
    </div>
  ),
})