import { createLazyFileRoute } from '@tanstack/react-router';
import DashboardComponent from '@/components/dashboard';

export const Route = createLazyFileRoute('/dashboard')({
  component: DashboardPage,
});

function DashboardPage() {
  return <DashboardComponent />;
}