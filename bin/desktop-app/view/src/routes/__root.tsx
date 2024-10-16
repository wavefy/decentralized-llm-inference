import { Toaster } from "@/components/ui/sonner";
import {
  createRootRoute,
  Link,
  Outlet,
  useMatchRoute,
} from "@tanstack/react-router";
import { TanStackRouterDevtools } from "@tanstack/router-devtools";

export const Route = createRootRoute({
  component: () => <RootComponent />,
});

const RootComponent = () => {
  const hideNavRoutes = ["/hidden"];

  const matchRoute = useMatchRoute();

  const matchedHideNavRoutes = hideNavRoutes.some((route) =>
    matchRoute({ to: route })
  );

  return (
    <div className="flex flex-col h-screen">
      {!matchedHideNavRoutes && (
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
              <Link
                to="/health"
                className="hover:text-gray-300 transition-colors duration-200 [&.active]:text-blue-400 [&.active]:font-semibold"
              >
                Health
                </Link>
            </div>
          </div>
        </nav>
      )}
      <div className="flex-1">
        <Outlet />
      </div>
      <Toaster />
      {/* <TanStackRouterDevtools /> */}
    </div>
  );
};
