import { fetchP2pStatus } from "@/api/p2p";
import { P2PStatus } from "@/lib/p2p";
import { useQueryClient, useQuery } from "@tanstack/react-query";

export const useP2PStatus = ({
  baseControlUrl,
}: {
  baseControlUrl: string;
}) => {
  const queryClient = useQueryClient();
  const { data: status, isLoading } = useQuery<P2PStatus>(
    {
      queryKey: ["P2P-STATUS"],
      queryFn: () => fetchP2pStatus(baseControlUrl),
      refetchInterval: 5000,
    },
    queryClient
  );

  return { isLoading, status };
};
