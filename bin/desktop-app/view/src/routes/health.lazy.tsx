import Health from '@/components/health'
import { createLazyFileRoute } from '@tanstack/react-router'

export const Route = createLazyFileRoute('/health')({
  component: Health,
})
