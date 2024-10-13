import { fetchSwarmHealth } from "@/api/health";
import { SwarmHealth } from "@/lib/health";
import { useQuery, useQueryClient } from "@tanstack/react-query";

export const useSwarmHealth = ({ registryUrl }: { registryUrl: string }) => {
  const queryClient = useQueryClient();
  const { data, isLoading } = useQuery<SwarmHealth[]>(
    {
      queryKey: ["SWARM-HEALTH"],
      queryFn: () => fetchSwarmHealth(registryUrl),
      refetchInterval: 5000,
    },
    queryClient
  );

  return { isLoading, data };
};
