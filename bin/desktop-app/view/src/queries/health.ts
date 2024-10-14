import { fetchSupportedModels, fetchSwarmHealth } from "@/api/health";
import { SwarmHealth } from "@/lib/health";
import { useQuery, useQueryClient } from "@tanstack/react-query";

export const useSwarmHealth = ({ registryUrl }: { registryUrl: string }) => {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery<SwarmHealth[]>(
    {
      queryKey: ["SWARM-HEALTH"],
      queryFn: () => fetchSwarmHealth(registryUrl),
      refetchInterval: 2500,
    },
    queryClient
  );

  return { isLoading, data };
};

export const useSupportedModels = ({ registryUrl }: { registryUrl: string }) => {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery<{ id: string, layers: number, memory: number }[]>(
    {
      queryKey: ["SUPPORTED-MODELS"],
      queryFn: () => fetchSupportedModels(registryUrl),
      refetchInterval: 5000,
    },
    queryClient
  );

  return { isLoading, data };
}